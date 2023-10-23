use std::fs::{self, File};
use std::io::prelude::*;

use anyhow::{Context, Result, anyhow};
use flate2::read::ZlibDecoder;

pub fn init() -> Result<()> {
    fs::create_dir(".git").context("Create root dir")?;
    fs::create_dir(".git/objects").context("Create objects dir")?;
    fs::create_dir(".git/refs").context("Create refs dir")?;
    fs::write(".git/HEAD", "ref: refs/heads/master\n").context("Write HEAD")?;
    Ok(())
}


#[derive(Debug, Clone)]
pub struct ObjectHeader {
    pub data_type: String,
    pub data_length: usize,
}
impl TryFrom<Vec<u8>> for ObjectHeader {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self> {
        let header_str = std::str::from_utf8(&value).context("Parsing header bytes")?;
        let (data_type, data_length) = header_str.rsplit_once(' ').ok_or(anyhow!("Invalid header format"))?;
        let data_type = data_type.to_string();
        let data_length = data_length.parse()?;
        Ok(Self { data_type, data_length })
    }
}

#[derive(Debug)]
pub struct Object {
    pub header: ObjectHeader,
    pub data: Vec<u8>,
}

impl Object {
    pub fn read(object_hash: &str) -> Result<Self> {
        let (prefix, filename) = object_hash.split_at(2);
        let file = File::open(format!(".git/objects/{}/{}", prefix, filename))
            .context("Opening object file")?;
        let mut decoder = ZlibDecoder::new(file);
        let mut buf = Vec::new();
        decoder
            .read_to_end(&mut buf)
            .context("Reading object file")?;

        let mut buf = buf.into_iter();

        let header = buf.by_ref().take_while(|c| *c != b'\0').collect::<Vec<_>>().try_into()?;

        let data = buf.collect();

        Ok(Self { header, data })
    }

    pub fn print_pretty(&self) -> Result<()> {
        std::io::stdout().lock().write_all(&self.data).context("Writing result")?;
        Ok(())
    }
}
