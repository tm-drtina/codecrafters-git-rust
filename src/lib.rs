use std::fs::{self, File};
use std::io::prelude::*;

use anyhow::{anyhow, Context, Result};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use sha1::{Digest, Sha1};

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
        let (data_type, data_length) = header_str
            .rsplit_once(' ')
            .ok_or(anyhow!("Invalid header format"))?;
        let data_type = data_type.to_string();
        let data_length = data_length.parse()?;
        Ok(Self {
            data_type,
            data_length,
        })
    }
}
impl ObjectHeader {
    fn as_str(&self) -> String {
        format!("{} {}\0", self.data_type, self.data_length)
    }
}

#[derive(Debug)]
pub struct Object {
    pub hash: String,
    pub header: ObjectHeader,
    pub data: Vec<u8>,
}

impl Object {
    pub fn read(hash: String) -> Result<Self> {
        let (prefix, filename) = hash.split_at(2);
        let file = File::open(format!(".git/objects/{}/{}", prefix, filename))
            .context("Opening object file")?;
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

    pub fn write(&self) -> Result<()> {
        let (prefix, filename) = self.hash.split_at(2);
        fs::create_dir_all(format!(".git/objects/{}", prefix)).context("Creating object dirs")?;
        let file = File::create(format!(".git/objects/{}/{}", prefix, filename))
            .context("Creating object file")?;
        let mut encoder = ZlibEncoder::new(file, Compression::default());
        encoder.write_all(self.header.as_str().as_bytes()).context("Writing header")?;
        encoder.write_all(&self.data).context("Writing data")?;
        Ok(())
    }

    pub fn create(mut file: File) -> Result<Self> {
        let mut data = Vec::new();
        let data_length = file.read_to_end(&mut data).context("Reading input file")?;

        let header = ObjectHeader {
            data_type: String::from("blob"),
            data_length,
        };

        let mut hasher = Sha1::new();
        hasher.update(header.as_str());
        hasher.update(&data);

        let hash = hex::encode(hasher.finalize());

        Ok(Self { hash, header, data })
    }

    pub fn print_pretty(&self) -> Result<()> {
        std::io::stdout()
            .lock()
            .write_all(&self.data)
            .context("Writing result")?;
        Ok(())
    }
}
