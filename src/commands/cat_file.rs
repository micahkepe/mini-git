//! The `cat-file` command.
//!
//! See: <https://git-scm.com/docs/git-cat-file>
use anyhow::Context;
use flate2::read::ZlibDecoder;
use std::ffi::CStr;
use std::io::prelude::*;
use std::io::{self, BufReader};

/// Git object types
enum Kind {
    /// A blob is a file of arbitrary content.
    Blob,
    // Tree,
    // Commit,
}

/// Invoke the `cat-file` command.
/// See: <https://git-scm.com/docs/git-cat-file>
pub(crate) fn invoke(pretty_print: bool, object_hash: String) -> anyhow::Result<()> {
    anyhow::ensure!(
        pretty_print,
        "mode must be given without -p, and we don't support mode"
    );

    // TODO: support shortest-unique object hashes
    let f = std::fs::File::open(format!(
        ".git/objects/{}/{}",
        &object_hash[..2],
        &object_hash[2..]
    ))
    .context("open in ./git/objects")?;

    let z = ZlibDecoder::new(f);
    let mut z = BufReader::new(z);
    let mut buf = Vec::new();
    z.read_until(0, &mut buf)
        .context("read header from .git/objects")?;
    let header = CStr::from_bytes_with_nul(&buf).expect("there is one nul at the end");
    let header = header
        .to_str()
        .context(".git/objects file header isn't valid UTF-8")?;
    let Some((kind, size)) = header.split_once(' ') else {
        anyhow::bail!(".git/object file header did not start with a known type: '{header}'");
    };
    let kind = match kind {
        "blob" => Kind::Blob,
        _ => anyhow::bail!("we don't know how to deal with kind: '{kind}'"),
    };
    let size = size
        .parse::<u64>()
        .context(".git/objects file header has invalid size: {size}")?;
    let mut z = LimitReader {
        reader: z,
        limit: size as usize,
    };
    match kind {
        Kind::Blob => {
            let mut stdout = std::io::stdout().lock();
            let n =
                std::io::copy(&mut z, &mut stdout).context("write .git/objects files to stdout")?;
            anyhow::ensure!(
                n == size,
                ".git/objects file was not the expected size (expected: {size}, actual: {n})"
            );
        }
    }

    Ok(())
}

/// Wrapper type around a reader that sets an explicit limit on the number of bytes to read.
struct LimitReader<R> {
    /// Underlying reader (e.g., `BufReader<ZlibDecoder<File>>`).
    reader: R,
    /// The maximum number of bytes that can be read.
    limit: usize,
}

impl<R> Read for LimitReader<R>
where
    R: Read,
{
    fn read(&mut self, mut buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.len() > self.limit {
            buf = &mut buf[..self.limit + 1];
        }
        let n = self.reader.read(buf)?;
        if n > self.limit {
            return Err(io::Error::other("too many bytes"));
        }
        self.limit -= n;
        Ok(n)
    }
}
