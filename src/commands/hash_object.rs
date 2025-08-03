//! The `hash-object` command.
//!
//! See: <https://git-scm.com/docs/git-hash-object>
use anyhow::Context;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use sha1::{Digest, Sha1};
use std::io;
use std::io::prelude::*;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// Writes the contents of a file to the object database.
///
/// If the `--write` flag is passed, the contents of the file are written to the object database
/// and the hash of the object is returned. Otherwise, the hash of the file is written to stdout.
/// The hash is written as a hex string.
fn write_blob<W>(file: &Path, writer: W) -> anyhow::Result<String>
where
    W: Write,
{
    let size = std::fs::metadata(file)
        .with_context(|| format!("reading metadata for file {}", file.display()))?
        .size();
    let writer = ZlibEncoder::new(writer, Compression::default());
    let mut writer = HashWriter {
        writer,
        hasher: Sha1::new(),
    };
    write!(writer, "blob ")?;
    write!(writer, "{}\0", size)?;
    let mut file =
        std::fs::File::open(file).with_context(|| format!("opening file {}", file.display()))?;
    std::io::copy(&mut file, &mut writer).context("stream file into blob")?;
    let _ = writer.writer.finish()?;
    let hash = writer.hasher.finalize();
    Ok(hex::encode(hash))
}

/// Invoke the `hash-object` command.
/// See: <https://git-scm.com/docs/git-hash-object>
pub(crate) fn invoke(write: bool, file: &Path) -> anyhow::Result<()> {
    let hash = if write {
        let tmp = "tmp";
        let hash = write_blob(
            file,
            std::fs::File::create(tmp).context("construct temporary file for blob")?,
        )
        .context("write out blob object")?;
        std::fs::create_dir_all(format!(".git/objects/{}/", &hash[..2]))
            .context("create subdirectory of `.git/objects/`")?;
        std::fs::rename(tmp, format!(".git/objects/{}/{}", &hash[..2], &hash[2..]))
            .context("move tmp blob file into `.git/objects`")?;
        hash
    } else {
        write_blob(file, std::io::sink()).context("write out blob object")?
    };

    println!("{hash}");
    Ok(())
}

/// A writer that hashes the contents of a file using the SHA-1 algorithm.
struct HashWriter<W> {
    /// The underlying writer (e.g., `ZlibEncoder<File>`).
    writer: W,
    /// The hasher used to hash the contents of the file.
    hasher: Sha1,
}

impl<W> Write for HashWriter<W>
where
    W: Write,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.writer.write(buf)?;
        self.hasher.update(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
