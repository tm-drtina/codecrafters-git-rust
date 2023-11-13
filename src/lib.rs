use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure, anyhow};

use crate::tree::Tree;

pub mod commit;
pub mod http_protocol;
pub mod object;
pub mod tree;

pub struct GitRepo {
    pub repo_root: PathBuf,
    pub git_dir: PathBuf,
    pub objects_dir: PathBuf,
    pub refs_dir: PathBuf,
}

impl GitRepo {
    pub fn new(repo_root: &Path) -> Self {
        Self {
            repo_root: repo_root.to_path_buf(),
            git_dir: repo_root.join(".git"),
            objects_dir: repo_root.join(".git").join("objects"),
            refs_dir: repo_root.join(".git").join("refs"),
        }
    }

    pub fn new_in_cwd() -> Result<Self> {
        Ok(Self::new(&std::env::current_dir()?))
    }

    pub fn init(&self) -> Result<()> {
        fs::create_dir(&self.git_dir).context("Create git dir")?;
        fs::create_dir(&self.objects_dir).context("Create objects dir")?;
        fs::create_dir(&self.refs_dir).context("Create refs dir")?;
        fs::write(self.git_dir.join("HEAD"), "ref: refs/heads/master\n").context("Write HEAD")?;
        Ok(())
    }

    // TODO: move to commit
    pub fn checkout(&self, commit_hash: String) -> Result<()> {
        eprintln!("Checkout commit at {}", commit_hash);
        let commit_obj = object::Object::read(&self, commit_hash)?;
        ensure!(commit_obj.header.kind == object::ObjectKind::Commit, "Given hash does not represent commit");
        let tree_hash = commit_obj.data.split(|c| *c == b'\n').find_map(|line| line.strip_prefix(b"tree ")).ok_or(anyhow!("Commit doesn't contain tree reference"))?;
        let tree_hash = String::from_utf8(tree_hash.to_vec())?;
        eprintln!("Checkout tree at {}", tree_hash);
        let tree: Tree = object::Object::read(&self, tree_hash)?.try_into()?;
        tree.checkout(&self, &self.repo_root)?;
        Ok(())
    }
}
