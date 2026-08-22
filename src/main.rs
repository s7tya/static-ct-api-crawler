use std::{
    fmt::{self},
    fs::{self, File},
    io::{Read, Write},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::Client;
use url::Url;

pub struct StaticCtLogClient {
    monitoring_prefix: Url,
    client: Client,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Checkpoint {
    pub tree_size: usize,
    pub root_hash: Vec<u8>,
}

impl Checkpoint {
    pub fn new(tree_size: usize, root_hash: Vec<u8>) -> Self {
        Self {
            tree_size,
            root_hash,
        }
    }
}

impl fmt::Display for Checkpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let root_hash_base64 = STANDARD.encode(&self.root_hash);
        write!(f, "{} {}", self.tree_size, root_hash_base64)
    }
}

impl StaticCtLogClient {
    pub fn new(monitoring_prefix: impl Into<Url>) -> Self {
        let client = Client::new();

        Self {
            monitoring_prefix: monitoring_prefix.into(),
            client,
        }
    }

    pub async fn get_checkpoint(&self) -> anyhow::Result<Checkpoint> {
        let url = self.monitoring_prefix.join("checkpoint")?;
        let resp = self.client.get(url).send().await?;
        let text = resp.text().await?;
        let mut lines = text.split("\n");

        let tree_size = lines.nth(1).unwrap().parse().unwrap();
        let root_hash_base64: String = lines.next().unwrap().into();
        let root_hash = STANDARD.decode(root_hash_base64)?;

        Ok(Checkpoint::new(tree_size, root_hash))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = StaticCtLogClient::new(Url::parse(
        "https://mon.sycamore.ct.letsencrypt.org/2026h1/",
    )?);
    let checkpoint = client.get_checkpoint().await?;

    let path = "./checkpoint.txt";
    if fs::exists(path)? {
        let mut file = File::open(path)?;
        let mut prev_checkpoint_str = String::new();
        file.read_to_string(&mut prev_checkpoint_str)?;
        let (tree_size_str, root_hash_str) = prev_checkpoint_str.split_once(' ').unwrap();
        let prev_checkpoint = Checkpoint {
            tree_size: tree_size_str.parse()?,
            root_hash: STANDARD.decode(root_hash_str)?,
        };

        if checkpoint == prev_checkpoint {
            return Ok(());
        } else if checkpoint.tree_size < prev_checkpoint.tree_size
            || (prev_checkpoint.tree_size != checkpoint.tree_size
                && prev_checkpoint.root_hash == checkpoint.root_hash)
        {
            return Err(anyhow::anyhow!("invalid checkpoint"));
        }
    }

    let mut file = File::create("./checkpoint.txt")?;
    file.write_all(checkpoint.to_string().as_bytes())?;

    Ok(())
}
