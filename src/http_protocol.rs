use std::collections::VecDeque;

use anyhow::{anyhow, bail, ensure, Context, Result};
use reqwest::blocking::{Client, RequestBuilder};

pub struct GitHttpClient {
    client: Client,
    url: String,
}

impl GitHttpClient {
    pub fn new(url: String) -> Self {
        Self {
            client: Client::new(),
            url,
        }
    }

    fn send(&self, req: RequestBuilder, service: &str) -> Result<VecDeque<PktLine>> {
        let resp = req.query(&[("service", service)]).send()?;

        let content_type = resp
            .headers()
            .get("Content-Type")
            .ok_or(anyhow!("Missing Content-Type header"))?
            .to_str()
            .context("Cannot convert Content-Type header value to str")?;
        let expected_content_type = format!("application/x-{}-advertisement", service);
        ensure!(
            content_type == expected_content_type,
            "Expected git smart response content type"
        );

        let data = resp.bytes()?;
        let mut slice = data.as_ref();
        let mut lines = VecDeque::new();

        let mut data_len_bytes = [0u8; 2];
        while slice.len() > 0 {
            let (len, rest) = slice.split_at(4);
            hex::decode_to_slice(std::str::from_utf8(len)?, &mut data_len_bytes).context("Decoding data len hex")?;
            let data_len = u16::from_be_bytes(data_len_bytes) as usize;
            if data_len == 0 {
                slice = rest;
                lines.push_back(PktLine::Flush);
            } else {
                ensure!(
                    data_len >= 4,
                    "pkt-line length must be at least 4 to compensate for legth bytes"
                );
                let (data, rest) = rest.split_at(data_len - 4);
                slice = rest;
                lines.push_back(PktLine::Data(Box::from(data)));
            }
        }
        Ok(lines)
    }

    pub fn ref_info(&self) -> Result<RefInfo> {
        let service = "git-upload-pack";
        let service_bytes = service.as_bytes();
        let req = self.client.get(format!("{}/info/refs", self.url));

        let mut lines = self.send(req, service)?;
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
            let pos = data.iter().position(|x| *x == b'\0').ok_or(anyhow!("Missing null-byte in first data line"))?;
            let (refs_bytes, capabilities_bytes) = data.split_at(pos);
            let capabilities_bytes = &capabilities_bytes[1..];
            capabilities = capabilities_bytes.split(|x| *x == b' ').map(|s| std::str::from_utf8(s).map(String::from).context("Capabilities must be valid strs")).collect::<Result<_>>()?;

            if data.starts_with(b"0000000000000000000000000000000000000000") {
                ensure!(lines.pop_front() == Some(PktLine::Flush), "Data must end with flush line");
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
}

#[derive(Debug, PartialEq, Eq)]
enum PktLine {
    Data(Box<[u8]>),
    Flush,
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
