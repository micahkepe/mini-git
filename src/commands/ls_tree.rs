//! The `ls-tree` command.
//!
//! See <https://git-scm.com/docs/git-ls-tree>
use crate::objects::{Kind, Object};
use anyhow::Context;
use std::{
    ffi::CStr,
    io::{BufRead, Read, Write},
};

/// Invoke the `ls-tree` command.
pub(crate) fn invoke(name_only: bool, tree_sha: &str) -> anyhow::Result<()> {
    let mut object = Object::read(tree_sha).context("parse out tree object file")?;
    match object.kind {
        Kind::Tree => {
            let mut buf = Vec::new();
            let mut hash_buf = [0; 20];
            let mut stdout = std::io::stdout().lock();
            loop {
                buf.clear();
                let n = object
                    .reader
                    .read_until(0, &mut buf)
                    .context("read next tree object entry")?;
                if n == 0 {
                    break; // EOF
                }
                object
                    .reader
                    .read_exact(&mut hash_buf[..])
                    .context("read tree object entry hash into hash_buf")?;

                let mode_and_name =
                    CStr::from_bytes_with_nul(&buf).context("invalid tree entry")?;
                let mut bits = mode_and_name.to_bytes().splitn(2, |&b| b == b' ');
                let mode = bits.next().expect("split always yields once");
                let name = bits
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("tree entry has no file name"))?;

                if name_only {
                    stdout
                        .write_all(name)
                        .context("write tree entry name to stdout")?;
                    writeln!(stdout).context("write newline to stdout")?;
                } else {
                    let mode = std::str::from_utf8(mode).context("mode is always valid UTF-8")?;
                    let hash = hex::encode(hash_buf);
                    let object = Object::read(&hash)
                        .with_context(|| format!("read object for tree entry {hash}"))?;
                    write!(stdout, "{mode:0>6} {} {hash}\t", object.kind)
                        .context("write tree entry meta to stdout")?;
                    stdout
                        .write_all(name)
                        .context("write tree entry name to stdout")?;
                }
            }
        }
        _ => anyhow::bail!("cannot ls kind: {}", object.kind),
    }

    Ok(())
}
