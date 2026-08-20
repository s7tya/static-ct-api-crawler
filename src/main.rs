use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::Client;

pub struct StaticCtLogClient {
    base_url: String,
    client: Client,
}

#[derive(Debug)]
pub struct Checkpoint {
    tree_size: usize,
    root_hash: Vec<u8>,
}

impl StaticCtLogClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        let client = Client::new();
        let base_url = base_url.into();

        Self { base_url, client }
    }

    pub async fn get_checkpoint(&self) -> anyhow::Result<Checkpoint> {
        let resp = self
            .client
            .get(format!("{}/checkpoint", self.base_url))
            .send()
            .await?;
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
    let client = StaticCtLogClient::new("https://mon.sycamore.ct.letsencrypt.org/2026h1");
    let checkpoint = client.get_checkpoint().await?;
    println!("{:?}", checkpoint);

    Ok(())
}
