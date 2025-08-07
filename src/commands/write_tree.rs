//! The `write-tree` command.
//!
//! See: <https://git-scm.com/docs/git-write-tree>
use anyhow::Context;
use std::cmp::Ordering;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::{fs, io::Cursor};

use crate::objects::{Kind, Object};

/// Recursively write a tree object for a directory. Returns the hash of the tree object if it was
/// created, or `None` if the directory is empty.
///
/// NOTE: this uses Unix permissions to determine file mode, so it may not work on Windows.
pub(crate) fn write_tree_for(path: &Path) -> anyhow::Result<Option<[u8; 20]>> {
    let dir = fs::read_dir(path).with_context(|| format!("open directory {}", path.display()))?;
    let mut tree_object = Vec::new();

    let mut entries = Vec::new();
    for entry in dir {
        let entry = entry.with_context(|| format!("bad directory entry in {}", path.display()))?;
        let name = entry.file_name();
        let meta = entry.metadata().context("metadata for directory entry")?;
        entries.push((entry, name, meta))
    }
    // Sort file contents following Git's conventions:
    // See <https://github.com/git/git/blob/64cbe5e2e8a7b0f92c780b210e602496bd5cad0f/tree.c#L101>
    entries.sort_unstable_by(|a, b| {
        let afn = a.1.as_encoded_bytes();
        let bfn = b.1.as_encoded_bytes();
        let common_len = std::cmp::min(afn.len(), bfn.len());
        match afn[..common_len].cmp(&bfn[..common_len]) {
            Ordering::Equal => {}
            o => return o,
        }
        if afn.len() == bfn.len() {
            return Ordering::Equal;
        }
        let c1 = if let Some(c) = afn.get(common_len).copied() {
            Some(c)
        } else if a.2.is_dir() {
            Some(b'/')
        } else {
            None
        };

        let c2 = if let Some(c) = bfn.get(common_len).copied() {
            Some(c)
        } else if b.2.is_dir() {
            Some(b'/')
        } else {
            None
        };

        c1.cmp(&c2)
    });

    for (entry, file_name, meta) in entries {
        // Skip `.git/` entries
        if file_name == ".git" {
            continue;
        }

        // NOTE: these permissions are for UNIX systems
        let mode = if meta.is_dir() {
            "40000"
        } else if meta.is_symlink() {
            "120000"
        } else if (meta.permissions().mode() & 0o111) != 0 {
            // Has at least one executable bit set
            "100755"
        } else {
            // Regular file
            "100644"
        };
        let path = entry.path();
        let hash = if meta.is_dir() {
            let Some(hash) = write_tree_for(&path)? else {
                // Empty directory
                continue;
            };
            hash
        } else {
            let tmp = "tmp";
            let hash = Object::blob_from_file(&path)
                .context("open blob input file")?
                .write(std::fs::File::create(tmp).context("construct temporary file for blob")?)
                .context("stream file into blob")?;
            let hash_hex = hex::encode(hash);
            std::fs::create_dir_all(format!(".git/objects/{}/", &hash_hex[..2]))
                .context("create subdirectory of `.git/objects/`")?;
            std::fs::rename(
                tmp,
                format!(".git/objects/{}/{}", &hash_hex[..2], &hash_hex[2..]),
            )
            .context("move tmp blob file into `.git/objects`")?;
            hash
        };
        tree_object.extend(mode.as_bytes());
        tree_object.push(b' ');
        tree_object.extend(file_name.as_encoded_bytes());
        tree_object.push(0);
        tree_object.extend(hash);
    }

    if tree_object.is_empty() {
        Ok(None)
    } else {
        // Return created hash
        Ok(Some(
            Object {
                kind: Kind::Tree,
                expected_size: tree_object.len() as u64,
                reader: Cursor::new(tree_object),
            }
            .write_to_objects()
            .context("write tree object")?,
        ))
    }
}

/// Invoke the `write-tree` command.
/// See: <https://git-scm.com/docs/git-write-tree>
pub(crate) fn invoke() -> anyhow::Result<()> {
    let Some(hash) = write_tree_for(&std::env::current_dir()?)? else {
        anyhow::bail!("empty tree, no files to write");
    };

    println!("{}", hex::encode(hash));
    Ok(())
}
