use alloy::signers::local::LocalSigner;
use alloy::signers::Signer;
use std::str::FromStr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let pk = std::env::var("POLYGON_PRIVATE_KEY")?;
    let signer = LocalSigner::from_str(&pk)?;

    println!("🔑 Private Key Address: {}", signer.address());
    println!(
        "📍 Proxy Address from .env: {}",
        std::env::var("POLYMARKET_PROXY_ADDRESS").unwrap_or_default()
    );

    Ok(())
}
