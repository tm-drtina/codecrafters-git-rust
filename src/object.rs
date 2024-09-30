use std::fs::{self, File};
use std::io::prelude::*;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{anyhow, bail, ensure, Context, Result};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use sha1::{Digest, Sha1};

use crate::commit::Commit;
use crate::tree::Tree;
use crate::GitRepo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Blob,
    Commit,
    Tree,
}
impl ObjectKind {
    fn as_str(&self) -> &'static str {
        match self {
            ObjectKind::Blob => "blob",
            ObjectKind::Commit => "commit",
            ObjectKind::Tree => "tree",
        }
    }
}
impl FromStr for ObjectKind {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        Ok(match value {
            "blob" => Self::Blob,
            "commit" => Self::Commit,
            "tree" => Self::Tree,
            _ => bail!("Unrecognized object kind {:?}", value),
        })
    }
}

#[derive(Debug, Clone)]
pub struct ObjectHeader {
    pub kind: ObjectKind,
    pub data_length: usize,
}
impl TryFrom<Vec<u8>> for ObjectHeader {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self> {
        let header_str = std::str::from_utf8(&value).context("Parsing header bytes")?;
        let (data_type, data_length) = header_str
            .rsplit_once(' ')
            .ok_or(anyhow!("Invalid header format"))?;
        let kind = data_type.parse()?;
        let data_length = data_length.parse()?;
        Ok(Self { kind, data_length })
    }
}
impl ObjectHeader {
    fn as_str(&self) -> String {
        format!("{} {}\0", self.kind.as_str(), self.data_length)
    }
}

#[derive(Debug)]
pub struct Object {
    pub hash: String,
    pub header: ObjectHeader,
    pub data: Vec<u8>,
}

impl From<Tree> for Object {
    fn from(tree: Tree) -> Self {
        Self::new(ObjectKind::Tree, tree.to_bytes())
    }
}
impl From<Commit> for Object {
    fn from(commit: Commit) -> Self {
        Self::new(ObjectKind::Commit, commit.to_bytes())
    }
}
impl TryFrom<File> for Object {
    type Error = anyhow::Error;

    fn try_from(mut file: File) -> Result<Self> {
        let mut data = Vec::new();
        file.read_to_end(&mut data).context("Reading input file")?;
        Ok(Self::new(ObjectKind::Blob, data))
    }
}

impl Object {
    pub fn new(kind: ObjectKind, data: Vec<u8>) -> Self {
        let header = ObjectHeader {
            kind,
            data_length: data.len(),
        };

        let mut hasher = Sha1::new();
        hasher.update(header.as_str());
        hasher.update(&data);

        let hash = hex::encode(hasher.finalize());

        Self { hash, header, data }
    }

    pub(crate) fn path(repo: &GitRepo, hash: &str) -> (PathBuf, PathBuf) {
        let (prefix, filename) = hash.split_at(2);
        let dir_path = repo.objects_dir.join(prefix);
        let file_path = dir_path.join(filename);
        (dir_path, file_path)
    }

    pub fn read(repo: &GitRepo, hash: String) -> Result<Self> {
        let (_, path) = Self::path(repo, &hash);
        let file = File::open(path).context("Opening object file")?;
        let mut decoder = ZlibDecoder::new(file);
        let mut buf = Vec::new();
        decoder
            .read_to_end(&mut buf)
            .context("Reading object file")?;

        let mut buf = buf.into_iter();

        let header = buf
            .by_ref()
            .take_while(|c| *c != b'\0')
            .collect::<Vec<_>>()
            .try_into()?;

        let data = buf.collect();

        Ok(Self { hash, header, data })
    }

    pub fn write(&self, repo: &GitRepo) -> Result<()> {
        let (dir_path, file_path) = Self::path(repo, &self.hash);
        fs::create_dir_all(dir_path).context("Creating object dirs")?;
        let file = File::create(file_path).context("Creating object file")?;
        let mut encoder = ZlibEncoder::new(file, Compression::default());
        encoder
            .write_all(self.header.as_str().as_bytes())
            .context("Writing header")?;
        encoder.write_all(&self.data).context("Writing data")?;
        Ok(())
    }

    pub fn print_pretty(&self) -> Result<()> {
        ensure!(
            self.header.kind == ObjectKind::Blob,
            "Pretty print is supported for blobs only!"
        );
        std::io::stdout()
            .lock()
            .write_all(&self.data)
            .context("Writing result")?;
        Ok(())
    }
}
