//! The `cat-file` command.
//!
//! See: <https://git-scm.com/docs/git-cat-file>
use crate::objects::{Kind, Object};
use anyhow::Context;

/// Invoke the `cat-file` command.
/// See: <https://git-scm.com/docs/git-cat-file>
pub(crate) fn invoke(pretty_print: bool, object_hash: String) -> anyhow::Result<()> {
    anyhow::ensure!(
        pretty_print,
        "mode must be given without -p, and we don't support mode"
    );

    let mut object = Object::read(&object_hash).context("parse out blob object file")?;
    match object.kind {
        Kind::Blob => {
            let mut stdout = std::io::stdout().lock();
            let n = std::io::copy(&mut object.reader, &mut stdout)
                .context("write .git/objects files to stdout")?;
            anyhow::ensure!(
                n == object.expected_size,
                ".git/objects file was not the expected size (expected: {}, actual: {n})",
                object.expected_size
            );
        }
        _ => anyhow::bail!("haven't implemented printing out kind: {}", object.kind),
    }

    Ok(())
}
