use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use anyhow::{anyhow, bail, ensure, Context, Result};

use crate::object::{Object, ObjectKind};
use crate::GitRepo;

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

    fn set_permissions(file: &File, executable: bool) -> Result<()> {
        if cfg!(unix) {
            let metadata = file.metadata()?;
            let mut permissions = metadata.permissions();
            std::os::unix::fs::PermissionsExt::set_mode(
                &mut permissions,
                if executable { 0o755 } else { 0o644 },
            );
        } else {
            // We ignore permissions on non-unix systems
        }
        Ok(())
    }

    fn create_symlink(original: &Path, link: &Path) -> Result<()> {
        if cfg!(unix) {
            std::os::unix::fs::symlink(original, link)?;
        } else {
            bail!("Symlink on non-unix platforms are not supported");
        }
        Ok(())
    }

    pub fn checkout(&self, repo: &GitRepo, path: &Path) -> Result<()> {
        for entry in &self.entries {
            let subpath = path.join(&entry.name);
            match entry.mode.as_str() {
                "40000" => {
                    // dir
                    fs::create_dir(&subpath)?;
                    let subtree: Tree =
                        Object::read(repo, hex::encode(&entry.reference))?
                            .try_into()?;
                    subtree.checkout(repo, &subpath)?;
                }
                "120000" => {
                    // symlink
                    Self::create_symlink(
                        Path::new(&hex::encode(&entry.reference)),
                        &subpath,
                    )?;
                }
                "100644" | "100755" => {
                    // file
                    let mut file = File::create(subpath)?;
                    Self::set_permissions(&file, entry.mode == "100755")?;
                    let mut obj = Object::read(repo, hex::encode(&entry.reference))?;
                    file.write_all(&mut obj.data)?;
                    file.flush()?;
                }
                _ => {
                    bail!("Unrecognized filemode {}", entry.mode)
                }
            }
        }
        Ok(())
    }

    pub fn create(repo: &GitRepo, root: &Path) -> Result<Self> {
        ensure!(root.is_dir(), "Path must be directory");
        let mut entries = Vec::new();
        for item in fs::read_dir(root)? {
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
                let object = Self::write(repo, &item.path())?;
                entries.push(TreeEntry {
                    mode: String::from("40000"),
                    name,
                    reference: hex::decode(object.hash)?,
                })
            } else if file_type.is_file() {
                let object: Object = File::open(item.path())?.try_into()?;
                object.write(repo)?;

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

    pub fn write(repo: &GitRepo, path: &Path) -> Result<Object> {
        let obj = Self::create(repo, path)?.into_object();
        obj.write(repo)?;
        Ok(obj)
    }
}
