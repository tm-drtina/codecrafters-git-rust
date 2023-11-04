use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

pub mod commit;
pub mod http_protocol;
pub mod object;
pub mod tree;

pub fn init(base_path: &Path) -> Result<()> {
    let git_dir = base_path.join(".git");
    fs::create_dir(git_dir.as_path()).context("Create git dir")?;
    fs::create_dir(git_dir.join("objects")).context("Create objects dir")?;
    fs::create_dir(git_dir.join("refs")).context("Create refs dir")?;
    fs::write(git_dir.join("HEAD"), "ref: refs/heads/master\n").context("Write HEAD")?;
    Ok(())
}
