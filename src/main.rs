use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::Client;
use url::Url;

pub struct StaticCtLogClient {
    monitoring_prefix: Url,
    client: Client,
}

#[derive(Debug)]
pub struct Checkpoint {
    pub tree_size: usize,
    pub root_hash: Vec<u8>,
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

        Ok(Checkpoint {
            tree_size,
            root_hash,
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = StaticCtLogClient::new(Url::parse(
        "https://mon.sycamore.ct.letsencrypt.org/2026h1/",
    )?);
    let checkpoint = client.get_checkpoint().await?;
    println!("{:?}", checkpoint);

    Ok(())
}
