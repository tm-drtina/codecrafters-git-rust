use std::collections::{BTreeSet, VecDeque};
use std::io::Read;

use anyhow::{anyhow, bail, ensure, Context, Result};
use flate2::read::ZlibDecoder;
use reqwest::blocking::{Client, Response};

use crate::object::{Object, ObjectKind};
use crate::GitRepo;

pub struct GitHttpClient<'a> {
    repo: &'a GitRepo,
    client: Client,
    url: String,
}

impl<'a> GitHttpClient<'a> {
    pub fn new(repo: &'a GitRepo, url: String) -> Self {
        Self {
            repo,
            client: Client::new(),
            url,
        }
    }

    fn validate_content_type(&self, resp: &Response, content_type: &str) -> Result<()> {
        let actual_content_type = resp
            .headers()
            .get("Content-Type")
            .ok_or(anyhow!("Missing Content-Type header"))?
            .to_str()
            .context("Cannot convert Content-Type header value to str")?;
        ensure!(
            actual_content_type == content_type,
            "Unexpected Content-Type header value. Got {}",
            actual_content_type
        );
        Ok(())
    }

    fn load_varint(data: &mut &[u8]) -> u32 {
        let mut cont = true;
        let mut val = 0u32;
        let mut shift = 0;
        while cont {
            cont = data[0] >= 128;
            val += ((data[0] & 0b0111_1111) as u32) << shift;
            shift += 7;
            *data = &data[1..];
        }
        val
    }

    fn parse_pkt_lines(&self, mut lines: &[u8]) -> Result<VecDeque<PktLine>> {
        let mut pkt_lines = VecDeque::new();

        let mut data_len_bytes = [0u8; 2];
        while !lines.is_empty() {
            let (prefix, rest) = lines.split_at(4);
            if prefix == b"PACK" {
                let (version, rest) = rest.split_at(4);
                ensure!(version == [0, 0, 0, 2], "Packfile version should be 2");
                let (packets_num, mut rest) = rest.split_at(4);
                let packets_num = u32::from_be_bytes(packets_num.try_into()?);
                for _i in 0..packets_num {
                    let pack_entry_type = PackEntryType::try_from((rest[0] >> 4) & 0b0111)?;
                    let mut val = (rest[0] & 0b1111) as u32;
                    if rest[0] & 0b1000_0000 != 0 {
                        rest = &rest[1..];
                        val += Self::load_varint(&mut rest) << 4;
                    } else {
                        rest = &rest[1..];
                    }

                    match pack_entry_type {
                        PackEntryType::OBJ_COMMIT
                        | PackEntryType::OBJ_TREE
                        | PackEntryType::OBJ_BLOB
                        | PackEntryType::OBJ_TAG => {
                            let mut decoder = ZlibDecoder::new(rest);
                            let mut buf = Vec::new();
                            decoder
                                .read_to_end(&mut buf)
                                .context("Reading object file")?;
                            ensure!(val as usize == buf.len(), "Read incorrect number of bytes");
                            let read_bytes = decoder.total_in() as usize;

                            let kind = match pack_entry_type {
                                PackEntryType::OBJ_COMMIT => ObjectKind::Commit,
                                PackEntryType::OBJ_TREE => ObjectKind::Tree,
                                PackEntryType::OBJ_BLOB => ObjectKind::Blob,
                                PackEntryType::OBJ_TAG => todo!(),
                                PackEntryType::OBJ_OFS_DELTA | PackEntryType::OBJ_REF_DELTA => {
                                    unreachable!()
                                }
                            };

                            let obj = Object::new(kind, buf);
                            obj.write(self.repo)?;
                            eprintln!("{:?} {}", pack_entry_type, obj.hash);

                            rest = &rest[read_bytes..];
                        }
                        PackEntryType::OBJ_OFS_DELTA => {
                            todo!("OFS_DELTA object")
                        }
                        PackEntryType::OBJ_REF_DELTA => {
                            let ref_delta = hex::encode(&rest[..20]);
                            rest = &rest[20..];
                            eprintln!("REF_DELTA: {ref_delta}");

                            let mut decoder = ZlibDecoder::new(rest);
                            let mut buf = Vec::new();
                            decoder.read_to_end(&mut buf).context("Reading pack diff")?;
                            let read_bytes = decoder.total_in() as usize;
                            rest = &rest[read_bytes..];

                            let mut delta_data = &*buf;

                            let _source_len = Self::load_varint(&mut delta_data);
                            let target_len = Self::load_varint(&mut delta_data);

                            let source = Object::read(self.repo, ref_delta)?;
                            let mut output = Vec::<u8>::with_capacity(target_len as usize);

                            while !delta_data.is_empty() {
                                let op = delta_data[0];
                                delta_data = &delta_data[1..];

                                if op & 0b1000_0000 != 0 {
                                    // COPY
                                    let mut offset: u32 = 0;
                                    for i in 0..4 {
                                        if op & (0b0000_0001 << i) != 0 {
                                            offset += (delta_data[0] as u32) << (i * 8);
                                            delta_data = &delta_data[1..];
                                        }
                                    }
                                    let mut len: u32 = 0;
                                    for i in 0..3 {
                                        if op & (0b0001_0000 << i) != 0 {
                                            len += (delta_data[0] as u32) << (i * 8);
                                            delta_data = &delta_data[1..];
                                        }
                                    }

                                    eprintln!("Copy from: {offset} bytes: {len}");

                                    output.extend_from_slice(
                                        &source.data[offset as usize..(offset + len) as usize],
                                    );
                                } else {
                                    // INSERT
                                    let len = (op & 0b0111_1111) as usize;
                                    let insert_data = &delta_data[..len];
                                    delta_data = &delta_data[len..];

                                    eprintln!("Insert {len} bytes: {insert_data:?}");
                                    output.extend_from_slice(insert_data);
                                }
                            }

                            debug_assert_eq!(output.len(), target_len as usize);
                            Object::new(source.header.kind, output).write(self.repo)?;
                        }
                    }
                }
                let _checksum = &rest[..20];
                lines = &rest[20..];
                ensure!(lines.is_empty(), "Unexpected data after pack data");
            } else {
                hex::decode_to_slice(std::str::from_utf8(prefix)?, &mut data_len_bytes)
                    .context("Decoding data len hex")?;
                let data_len = u16::from_be_bytes(data_len_bytes) as usize;
                if data_len == 0 {
                    lines = rest;
                    pkt_lines.push_back(PktLine::Flush);
                } else {
                    ensure!(
                        data_len >= 4,
                        "pkt-line length must be at least 4 to compensate for legth bytes"
                    );
                    let (data, rest) = rest.split_at(data_len - 4);
                    lines = rest;
                    pkt_lines.push_back(PktLine::Data(Box::from(data)));
                }
            }
        }
        Ok(pkt_lines)
    }

    pub fn ref_info(&self) -> Result<RefInfo> {
        let service = "git-upload-pack";
        let service_bytes = service.as_bytes();
        let resp = self
            .client
            .get(format!("{}/info/refs", self.url))
            .query(&[("service", service)])
            .send()?;
        self.validate_content_type(&resp, &format!("application/x-{}-advertisement", service))?;

        let mut lines = self.parse_pkt_lines(&resp.bytes()?)?;
        if let Some(PktLine::Data(data)) = lines.pop_front() {
            ensure!(
                data.len() == 10 + service_bytes.len()
                    || (data.len() == 10 + service_bytes.len() + 1 && data.last() == Some(&b'\n')),
                "Invalid header line"
            );
            ensure!(&data[..10] == b"# service=", "Invalid header prefix");
            ensure!(
                &data[10..(10 + service_bytes.len())] == service_bytes,
                "Invalid header value"
            );
        } else {
            bail!("Invalid header line");
        }
        ensure!(lines.pop_front() == Some(PktLine::Flush));

        let mut refs = Vec::new();

        fn parse_line(refs: &mut Vec<Ref>, mut data: &[u8]) -> Result<()> {
            if data.last() == Some(&b'\n') {
                data = &data[..data.len() - 1];
            }
            let id = &data[..40];
            let name = &data[41..];
            ensure!(data[40] == b' ');
            if name.ends_with(b"^{}") {
                let l = refs
                    .last_mut()
                    .ok_or(anyhow!("Peeled ref cannot be the first entry"))?;
                ensure!(l.name.as_bytes() == &name[..name.len() - 3]);
                ensure!(l.peeled_ref.is_none());
                l.peeled_ref = Some(id.try_into()?);
            } else {
                let name = std::str::from_utf8(&data[41..])?.to_string();
                refs.push(Ref {
                    name,
                    id: id.try_into()?,
                    peeled_ref: None,
                })
            }
            Ok(())
        }

        let capabilities;

        if let PktLine::Data(data) = lines
            .pop_front()
            .ok_or(anyhow!("Missing first data line"))?
        {
            let pos = data
                .iter()
                .position(|x| *x == b'\0')
                .ok_or(anyhow!("Missing null-byte in first data line"))?;
            let (refs_bytes, capabilities_bytes) = data.split_at(pos);
            let capabilities_bytes = &capabilities_bytes[1..];
            capabilities = capabilities_bytes
                .split(|x| *x == b' ')
                .map(|s| {
                    std::str::from_utf8(s)
                        .map(String::from)
                        .context("Capabilities must be valid strs")
                })
                .collect::<Result<_>>()?;

            if data.starts_with(b"0000000000000000000000000000000000000000") {
                ensure!(
                    lines.pop_front() == Some(PktLine::Flush),
                    "Data must end with flush line"
                );
                ensure!(lines.is_empty(), "Unexpected data after last flush line");
                return Ok(RefInfo { capabilities, refs });
            } else {
                parse_line(&mut refs, refs_bytes)?;
            }
        } else {
            bail!("Invalid first data line");
        };

        while let Some(PktLine::Data(data)) = lines.pop_front() {
            parse_line(&mut refs, &data)?;
        }
        ensure!(lines.is_empty(), "Unexpected data after last flush line");

        Ok(RefInfo { capabilities, refs })
    }

    pub fn fetch_refs(&self, refs: BTreeSet<&[u8; 40]>) -> Result<()> {
        let mut body = Vec::with_capacity(refs.len() * 50 + 4 + 9);
        for r in refs {
            body.extend(b"0032want ");
            body.extend_from_slice(r);
            body.push(b'\n');
        }
        body.extend(b"0000");
        body.extend(b"0009done\n");

        let resp = self
            .client
            .post(format!("{}/git-upload-pack", self.url))
            .header("Content-Type", "application/x-git-upload-pack-request")
            .body(body)
            .send()?;

        self.validate_content_type(&resp, "application/x-git-upload-pack-result")?;

        let mut lines = self.parse_pkt_lines(&resp.bytes()?)?;
        ensure!(lines.pop_front() == Some(PktLine::Data(Box::from(*b"NAK\n"))));
        ensure!(lines.is_empty());

        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
enum PktLine {
    Data(Box<[u8]>),
    Flush,
}

#[allow(non_camel_case_types)]
#[derive(Debug)]
pub enum PackEntryType {
    OBJ_COMMIT,
    OBJ_TREE,
    OBJ_BLOB,
    OBJ_TAG,
    OBJ_OFS_DELTA,
    OBJ_REF_DELTA,
}

impl TryFrom<u8> for PackEntryType {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self> {
        Ok(match value {
            0 => bail!("Forbidden value"),
            1 => Self::OBJ_COMMIT,
            2 => Self::OBJ_TREE,
            3 => Self::OBJ_BLOB,
            4 => Self::OBJ_TAG,
            5 => bail!("Reserved value"),
            6 => Self::OBJ_OFS_DELTA,
            7 => Self::OBJ_REF_DELTA,
            _ => unreachable!(),
        })
    }
}

pub struct Ref {
    pub name: String,
    pub id: [u8; 40],
    pub peeled_ref: Option<[u8; 40]>,
}

pub struct RefInfo {
    pub capabilities: Vec<String>,
    pub refs: Vec<Ref>,
}
