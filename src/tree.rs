use anyhow::{anyhow, ensure, Context, Result};

use crate::object::{Object, ObjectKind};

pub struct TreeEntry {
    pub mode: String,
    pub name: String,
    pub reference: [u8; 20],
}

pub struct Tree {
    pub content_size: usize,
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
            let reference = data[split + 1..split + 21].try_into()?;
            data = &data[split + 21..];
            entries.push(TreeEntry {
                mode: mode.to_string(),
                name: name.to_string(),
                reference,
            });
        }

        Ok(Self {
            content_size: object.header.data_length,
            entries,
        })
    }
}
