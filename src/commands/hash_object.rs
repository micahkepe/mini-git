//! The `hash-object` command.
//!
//! See: <https://git-scm.com/docs/git-hash-object>
use anyhow::Context;
use std::path::Path;

use crate::objects::Object;

/// Invoke the `hash-object` command.
/// See: <https://git-scm.com/docs/git-hash-object>
///
/// If the `--write` flag is passed, the contents of the file are written to the object database
/// and the hash of the object is returned. Otherwise, the hash of the file is written to stdout.
/// The hash is written as a hex string.
pub(crate) fn invoke(write: bool, file: &Path) -> anyhow::Result<()> {
    let object = Object::blob_from_file(file).context("open blob input file")?;
    let hash = if write {
        object
            .write_to_objects()
            .context("stream file into blob object file")?
    } else {
        object
            .write(std::io::sink())
            .context("stream file into blob object")?
    };

    println!("{}", hex::encode(hash));

    Ok(())
}
