//! The `ls-tree` command.
//!
//! See <https://git-scm.com/docs/git-ls-tree>

/// Invoke the `ls-tree` command.
pub(crate) fn invoke(name_only: bool, tree_sha: &str) -> anyhow::Result<()> {
    anyhow::ensure!(name_only, "Only --name-only supported for now");

    todo!();
}
