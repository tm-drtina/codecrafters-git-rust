use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct Author {
    pub name: String,
    pub email: String,
    pub time: SystemTime,
    pub time_offset: String,
}

impl Author {
    fn write_to_buf(&self, buf: &mut Vec<u8>) {
        buf.extend(self.name.as_bytes());
        buf.extend(b" <");
        buf.extend(self.email.as_bytes());
        buf.extend(b"> ");
        buf.extend(
            self.time
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("SystemTime before epoch")
                .as_secs()
                .to_string()
                .as_bytes(),
        );
        buf.push(b' ');
        buf.extend(self.time_offset.as_bytes());
    }
}

#[derive(Debug, Clone)]
pub struct Commit {
    pub tree_sha: String,
    pub parent: Option<String>,
    pub author: Author,
    pub commiter: Author,
    pub message: String,
}

impl Commit {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::new();

        data.extend(b"tree ");
        data.extend(self.tree_sha.as_bytes());
        data.push(b'\n');

        if let Some(ref parent) = self.parent {
            data.extend(b"parent ");
            data.extend(parent.as_bytes());
            data.push(b'\n');
        }

        data.extend(b"author ");
        self.author.write_to_buf(&mut data);
        data.push(b'\n');

        data.extend(b"commiter ");
        self.author.write_to_buf(&mut data);
        data.push(b'\n');

        data.push(b'\n');

        data.extend(self.message.as_bytes());
        data.push(b'\n');

        data
    }
}
