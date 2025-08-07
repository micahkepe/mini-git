//! The `commit-tree` command.
//!
//! See: <https://git-scm.com/docs/git-commit-tree>
use anyhow::Context;
use chrono::Local;
use std::fmt::Write;
use std::io::Cursor;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::objects::{Kind, Object};

/// Get the value of a Git config key, e.g. `user.name`.
fn get_git_config_value(key: &str) -> anyhow::Result<String> {
    let output = std::process::Command::new("git")
        .args(["config", "--get", key])
        .output()
        .context("get git config value")?;
    if !output.status.success() {
        anyhow::bail!("git config failed for key: {key}");
    }
    Ok(String::from_utf8(output.stdout)
        .context("Invalid UTF-8 from git config")?
        .trim()
        .to_string())
}

/// Write a commit object to the `.git/objects` directory.
pub(crate) fn write_commit(
    message: &str,
    tree_hash: &str,
    parent_hash: Option<&str>,
) -> anyhow::Result<[u8; 20]> {
    let mut commit = String::new();
    writeln!(commit, "tree {tree_hash}")?;
    if let Some(parent_hash) = parent_hash {
        writeln!(commit, "parent {parent_hash}")?;
    }

    let name = get_git_config_value("user.name")?;
    let email = get_git_config_value("user.email")?;
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let utc_offset = {
        let now = Local::now();
        now.format("%z").to_string()
    };

    writeln!(
        commit,
        "author {} <{}> {} {}",
        name, email, timestamp, utc_offset
    )?;
    writeln!(
        commit,
        "committer {} <{}> {} {}",
        name, email, timestamp, utc_offset
    )?;
    writeln!(commit)?;
    writeln!(commit, "{message}")?;

    Object {
        kind: Kind::Commit,
        expected_size: commit.len() as u64,
        reader: Cursor::new(commit),
    }
    .write_to_objects()
    .context("write commit object")
}

/// Invoke the `commit-tree` command.
/// See: <https://git-scm.com/docs/git-commit-tree>
pub(crate) fn invoke(
    message: String,
    tree_hash: String,
    parent_hash: Option<String>,
) -> anyhow::Result<()> {
    let hash =
        write_commit(&message, &tree_hash, parent_hash.as_deref()).context("write commit")?;
    println!("{}", hex::encode(hash));
    Ok(())
}
