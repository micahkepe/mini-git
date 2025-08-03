use anyhow::Context;
use clap::{Parser, Subcommand};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use sha1::{Digest, Sha1};
use std::ffi::CStr;
use std::fs;
use std::io;
use std::io::prelude::*;
use std::io::BufReader;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Initialize a new Git repository
    Init,
    /// See the contents of a Git object
    CatFile {
        /// The name of the object to show.
        object_hash: String,
        #[clap(short = 'p')]
        /// Pretty-print the contents of <object> based on its type.
        pretty_print: bool,
        // TODO: support for other flags: `-t`, `-s`, `-e`
    },
    /// Compute the SHA-1 has of a Git Object and optionally write to `.git/objects/`
    HashObject {
        /// Whether to write the object to `.git/objects` directory
        #[clap(short = 'w')]
        write: bool,
        /// The file to hash
        file: PathBuf,
        // TODO: support reading in from standard input with `--stdin`
    },
}

/// Git object types
enum Kind {
    Blob,
    // Tree,
    // Commit,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Init => {
            fs::create_dir(".git").unwrap();
            fs::create_dir(".git/objects").unwrap();
            fs::create_dir(".git/refs").unwrap();
            fs::write(".git/HEAD", "ref: refs/heads/main\n").unwrap();
            println!("Initialized git directory")
        }
        Command::CatFile {
            pretty_print,
            object_hash,
        } => {
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
                anyhow::bail!(
                    ".git/object file header did not start with a known type: '{header}'"
                );
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
                    let n = std::io::copy(&mut z, &mut stdout)
                        .context("write .git/objects files to stdout")?;
                    anyhow::ensure!(
                        n == size,
                        ".git/objects file was not the expected size (expected: {size}, actual: {n})"
                    );
                }
            }
        }
        Command::HashObject { write, file } => {
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
                let mut file = std::fs::File::open(file)
                    .with_context(|| format!("opening file {}", file.display()))?;
                std::io::copy(&mut file, &mut writer).context("stream file into blob")?;
                let _ = writer.writer.finish()?;
                let hash = writer.hasher.finalize();
                Ok(hex::encode(hash))
            }

            let hash = if write {
                let tmp = "tmp";
                let hash = write_blob(
                    &file,
                    std::fs::File::create(tmp).context("construct temporary file for blob")?,
                )
                .context("write out blob object")?;
                std::fs::create_dir_all(format!(".git/objects/{}/", &hash[..2]))
                    .context("create subdirectory of `.git/objects/`")?;
                std::fs::rename(tmp, format!(".git/objects/{}/{}", &hash[..2], &hash[2..]))
                    .context("move tmp blob file into `.git/objects`")?;
                hash
            } else {
                write_blob(&file, std::io::sink()).context("write out blob object")?
            };

            println!("{hash}")
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

struct HashWriter<W> {
    writer: W,
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
