//! Git objects definitions.
use anyhow::Context;
use flate2::read::ZlibDecoder;
use std::ffi::CStr;
use std::fmt;
use std::io::prelude::*;
use std::io::{self, BufReader};

/// Git object types
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Kind {
    /// A blob is a file of arbitrary content.
    Blob,
    /// A tree object is a directory listing of a set of objects.
    Tree,
    /// A commit object is a set of metadata and file system changes associated with a particular
    /// snapshot of the project's source code.
    Commit,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Blob => write!(f, "blob"),
            Kind::Tree => write!(f, "tree"),
            Kind::Commit => write!(f, "commit"),
        }
    }
}

/// Represents a Git object with its kind, expected size, and a reader over its contents.
pub(crate) struct Object<R> {
    /// The type of Git object.
    pub(crate) kind: Kind,
    /// Expected size of the object.
    pub(crate) expected_size: u64,
    /// Remaining bytes to read.
    pub(crate) reader: R,
}

impl Object<()> {
    pub(crate) fn read(hash: &str) -> anyhow::Result<Object<impl BufRead>> {
        // TODO: support shortest-unique object hashes
        let f = std::fs::File::open(format!(".git/objects/{}/{}", &hash[..2], &hash[2..]))
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
            "tree" => Kind::Tree,
            "commit" => Kind::Commit,
            _ => anyhow::bail!("unknown Git object kind: '{kind}'"),
        };
        let size = size
            .parse::<u64>()
            .context(".git/objects file header has invalid size: {size}")?;
        let z = LimitReader {
            reader: z,
            limit: size as usize,
        };

        Ok(Object {
            kind,
            expected_size: size,
            reader: z,
        })
    }
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

impl<R> BufRead for LimitReader<R>
where
    R: BufRead,
{
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        let buf = self.reader.fill_buf()?;
        if buf.len() > self.limit {
            Ok(&buf[..self.limit])
        } else {
            Ok(buf)
        }
    }

    fn consume(&mut self, amount: usize) {
        let amount = amount.min(self.limit);
        self.reader.consume(amount);
        self.limit -= amount;
    }
}
