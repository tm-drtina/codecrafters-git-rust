use std::fs::{self, File};
use std::path::Path;

use anyhow::{anyhow, bail, ensure, Context, Result};

use crate::object::{Object, ObjectKind};

pub struct TreeEntry {
    pub mode: String,
    pub name: String,
    pub reference: Vec<u8>,
}

pub struct Tree {
    pub entries: Vec<TreeEntry>,
}

impl TryFrom<Object> for Tree {
    type Error = anyhow::Error;

    fn try_from(object: Object) -> Result<Self> {
        ensure!(
            object.header.kind == ObjectKind::Tree,
            "Invalid object kind"
        );
        let mut entries = Vec::new();

        let mut data = &*object.data;
        loop {
            let split = if let Some(pos) = data.iter().position(|c| *c == b'\0') {
                pos
            } else {
                break;
            };
            let (mode, name) = std::str::from_utf8(&data[..split])
                .context("Parsing entry header")?
                .split_once(" ")
                .ok_or(anyhow!("Invalid entry header"))?;
            let reference = data[split + 1..split + 21].to_vec();
            data = &data[split + 21..];
            entries.push(TreeEntry {
                mode: mode.to_string(),
                name: name.to_string(),
                reference,
            });
        }

        Ok(Self { entries })
    }
}

impl Tree {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::new();
        for entry in &self.entries {
            data.extend(entry.mode.as_bytes());
            data.push(b' ');
            data.extend(entry.name.as_bytes());
            data.push(b'\0');
            data.extend(&entry.reference);
        }
        data
    }

    pub fn into_object(&self) -> Object {
        Object::new(ObjectKind::Tree, self.to_bytes())
    }

    fn filemode(d: &fs::DirEntry) -> Result<String> {
        Ok(if cfg!(unix) {
            if 0o100 & std::os::unix::fs::PermissionsExt::mode(&d.metadata()?.permissions()) > 0 {
                String::from("100755")
            } else {
                String::from("100644")
            }
        } else {
            String::from("100644")
        })
    }

    pub fn create(dir: &Path) -> Result<Self> {
        ensure!(dir.is_dir(), "Path must be directory");
        let mut entries = Vec::new();
        for item in fs::read_dir(dir)? {
            let item = item?;
            let file_type = item.file_type()?;
            let name = item
                .file_name()
                .into_string()
                .map_err(|s| anyhow!("Cannot convert filename into str: {:?}", s))?;
            if file_type.is_dir() {
                if name == ".git" {
                    continue;
                }
                let object = Self::write(&item.path())?;
                entries.push(TreeEntry {
                    mode: String::from("40000"),
                    name,
                    reference: hex::decode(object.hash)?,
                })
            } else if file_type.is_file() {
                let object: Object = File::open(item.path())?.try_into()?;
                object.write()?;

                entries.push(TreeEntry {
                    mode: Self::filemode(&item)?,
                    name,
                    reference: hex::decode(object.hash)?,
                })
            } else if file_type.is_symlink() {
                let reference = item
                    .path()
                    .read_link()?
                    .as_os_str()
                    .to_str()
                    .ok_or(anyhow!("Failed to read link as str"))?
                    .as_bytes()
                    .to_vec();
                entries.push(TreeEntry {
                    mode: String::from("120000"),
                    name,
                    reference,
                });
            } else {
                bail!("Unsupported file type {:?}", file_type);
            }
        }

        entries.sort_unstable_by(|a, b| a.name.cmp(&b.name));

        Ok(Self { entries })
    }

    pub fn write(dir: &Path) -> Result<Object> {
        let obj = Self::create(dir)?.into_object();
        obj.write()?;
        Ok(obj)
    }
}
