use std::fs;

use anyhow::{Context, Result};

pub mod commit;
pub mod object;
pub mod tree;

pub fn init() -> Result<()> {
    fs::create_dir(".git").context("Create root dir")?;
    fs::create_dir(".git/objects").context("Create objects dir")?;
    fs::create_dir(".git/refs").context("Create refs dir")?;
    fs::write(".git/HEAD", "ref: refs/heads/master\n").context("Write HEAD")?;
    Ok(())
}
