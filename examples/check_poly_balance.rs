use alloy::signers::local::LocalSigner;
use alloy::signers::Signer;
use polymarket_client_sdk::clob::{Client, Config};
use polymarket_client_sdk::POLYGON;
use std::str::FromStr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let pk = std::env::var("POLYGON_PRIVATE_KEY")?;
    let signer = LocalSigner::from_str(&pk)?.with_chain_id(Some(POLYGON));

    println!("🔑 Address: {}", signer.address());

    // Create authenticated client
    let client = Client::new("https://clob.polymarket.com", Config::default())?
        .authentication_builder(&signer)
        .authenticate()
        .await?;

    // Check balance via Polymarket API
    println!("\n📊 Checking balance via Polymarket API...");
    match client.balance_allowance(Default::default()).await {
        Ok(balance) => {
            println!("✅ Balance response: {:#?}", balance);
        }
        Err(e) => {
            println!("❌ Failed to get balance: {}", e);
        }
    }

    Ok(())
}
