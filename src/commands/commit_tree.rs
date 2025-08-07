//! The `commit-tree` command.
//!
//! See: <https://git-scm.com/docs/git-commit-tree>
use anyhow::Context;
use std::fmt::Write;
use std::io::Cursor;

use crate::objects::{Kind, Object};

pub(crate) fn invoke(
    message: String,
    tree_hash: String,
    parent_hash: Option<String>,
) -> anyhow::Result<()> {
    let mut commit = String::new();
    writeln!(commit, "tree {tree_hash}")?;
    if let Some(parent_hash) = parent_hash {
        writeln!(commit, "parent {parent_hash}")?;
    }
    // NOTE: hard coded author and committer information for now
    writeln!(
        commit,
        "author Micah Kepe <micahkepe@gmail.com> 1754538700000 -0700"
    )?;
    writeln!(
        commit,
        "committer Micah Kepe <micahkepe@gmail.com> 1754538700000 -0700"
    )?;
    writeln!(commit)?;
    writeln!(commit, "{message}")?;

    let hash = Object {
        kind: Kind::Commit,
        expected_size: commit.len() as u64,
        reader: Cursor::new(commit),
    }
    .write_to_objects()
    .context("write commit object")?;

    println!("{}", hex::encode(hash));
    Ok(())
}
