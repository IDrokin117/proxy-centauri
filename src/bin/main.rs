use anyhow::Result;
use proxima_centauri::Server;

#[tokio::main]
async fn main() -> Result<()> {
    Server::run().await?;
    Ok(())
}
