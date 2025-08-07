//! A (mini) Git implementation in Rust.
use anyhow::{Context, Ok};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;

pub(crate) mod commands;
pub(crate) mod objects;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

/// Git subcommands.
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
    /// Inspect a tree object.
    LsTree {
        /// Lists file contents by name only, excluding mode, object type, and object hashes.
        #[clap(long)]
        name_only: bool,
        /// The hash of the tree object to list.
        tree_sha: String,
    },
    /// Create a tree object from the current state of the staging area.
    WriteTree {},
    /// Create a commit object.
    CommitTree {
        /// Commit message.
        #[clap(short = 'm')]
        message: String,

        /// Parent tree
        #[clap(short = 'p')]
        parent_hash: Option<String>,

        /// Hash of tree object to commit.
        tree_hash: String,
    },
    /// Record changes to the repository.
    Commit {
        /// Commit message.
        #[clap(short = 'm')]
        message: String,
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
        Command::LsTree {
            name_only,
            tree_sha,
        } => commands::ls_tree::invoke(name_only, &tree_sha)?,
        Command::WriteTree {} => commands::write_tree::invoke()?,
        Command::CommitTree {
            message,
            tree_hash,
            parent_hash,
        } => commands::commit_tree::invoke(message, tree_hash, parent_hash)?,
        Command::Commit { message } => {
            let head_ref = fs::read_to_string(".git/HEAD").context("read HEAD")?;
            let Some(head_ref) = head_ref.strip_prefix("ref: ") else {
                anyhow::bail!("refusing to commit onto detached HEAD");
            };
            let head_ref = head_ref.trim();
            let parent_hash = std::fs::read_to_string(format!(".git/{head_ref}"))
                .with_context(|| format!("read HEAD reference target {head_ref}"))?;
            let parent_hash = parent_hash.trim();
            let Some(tree_hash) = commands::write_tree::write_tree_for(&std::env::current_dir()?)
                .context("write tree")?
            else {
                eprintln!("not committing empty tree");
                return Ok(());
            };
            let commit_hash = commands::commit_tree::write_commit(
                &message,
                &hex::encode(tree_hash),
                Some(parent_hash),
            )
            .context("create commit")?;
            let commit_hash = hex::encode(commit_hash);

            std::fs::write(format!(".git/{head_ref}"), &commit_hash)
                .with_context(|| format!("update HEAD reference target {head_ref}"))?;

            println!("HEAD is now at {commit_hash}")
        }
    }

    Ok(())
}
