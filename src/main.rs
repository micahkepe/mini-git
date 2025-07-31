use anyhow::Context;
use clap::{Parser, Subcommand};
use flate2::read::ZlibDecoder;
use std::ffi::CStr;
use std::fs;
use std::io::prelude::*;
use std::io::BufReader;

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
        #[clap(short = 'p')]
        pretty_print: bool,

        object_hash: String,
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
                .parse::<usize>()
                .context(".git/objects file header has invalid size: {size}")?;
            buf.clear();
            buf.resize(size, 0);
            z.read_exact(&mut buf[..])
                .context("read true contents of .git/objects file")?;
            let n = z
                .read(&mut [0])
                .context("validate EOF in .git/objects file")?;
            anyhow::ensure!(n == 0, ".git/objects file had {n} trailing bytes");
            let mut stdout = std::io::stdout().lock();
            match kind {
                Kind::Blob => {
                    stdout
                        .write_all(&buf)
                        .context("wrtie object contents to stdout")?;
                }
            }
        }
    }

    Ok(())
}
