//! Git objects definitions.
use anyhow::Context;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use sha1::{Digest, Sha1};
use std::ffi::CStr;
use std::fmt;
use std::io::prelude::*;
use std::io::{self, BufReader};
use std::path::Path;

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
    /// Create a new object from a file (blob).
    pub(crate) fn blob_from_file(file: impl AsRef<Path>) -> anyhow::Result<Object<impl Read>> {
        let file = file.as_ref();
        let size = std::fs::metadata(file)
            .with_context(|| format!("reading metadata for file {}", file.display()))?
            .len();
        // TODO: potential race here if file data changes between initial metadata fetch and writing
        // the blob
        let file = std::fs::File::open(file)
            .with_context(|| format!("opening file {}", file.display()))?;
        Ok(Object {
            kind: Kind::Blob,
            expected_size: size,
            reader: file,
        })
    }

    /// Read an object from the object store.
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

impl<R> Object<R>
where
    R: Read,
{
    /// Write the object to a writer.
    pub(crate) fn write(mut self, writer: impl Write) -> anyhow::Result<[u8; 20]> {
        let writer = ZlibEncoder::new(writer, Compression::default());
        let mut writer = HashWriter {
            writer,
            hasher: Sha1::new(),
        };
        write!(writer, "{} {}\0", self.kind, self.expected_size)?;
        std::io::copy(&mut self.reader, &mut writer).context("stream file into blob")?;
        let _ = writer.writer.finish()?;
        let hash = writer.hasher.finalize();
        Ok(hash.into())
    }

    /// Write the object to the `.git/objects` directory.
    pub(crate) fn write_to_objects(self) -> anyhow::Result<[u8; 20]> {
        let tmp = "tmp";
        let hash = self
            .write(std::fs::File::create(tmp).context("construct temporary file for tree")?)
            .context("stream tree object into file")?;

        let hash_hex = hex::encode(hash);
        std::fs::create_dir_all(format!(".git/objects/{}/", &hash_hex[..2]))
            .context("create subdirectory of `.git/objects/`")?;
        std::fs::rename(
            tmp,
            format!(".git/objects/{}/{}", &hash_hex[..2], &hash_hex[2..]),
        )
        .context("move tmp tree file into `.git/objects`")?;
        Ok(hash)
    }
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
