//! A (mini) Git implementation in Rust.
use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;

pub(crate) mod commands;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

/// Subcommands
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
        } => commands::cat_file::invoke(pretty_print, object_hash)?,
        Command::HashObject { write, file } => commands::hash_object::invoke(write, &file)?,
    }

    Ok(())
}
