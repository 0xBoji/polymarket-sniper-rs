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

    // Check balance BEFORE update
    println!("\n📊 Balance BEFORE update:");
    match client.balance_allowance(Default::default()).await {
        Ok(balance) => {
            println!("   Balance: {}", balance.balance);
            println!("   Allowances: {:?}", balance.allowances);
        }
        Err(e) => {
            println!("   ❌ Failed: {}", e);
        }
    }

    // UPDATE balance and allowance (this syncs with on-chain deposits)
    println!("\n🔄 Updating balance and allowance...");
    match client.update_balance_allowance(Default::default()).await {
        Ok(result) => {
            println!("   ✅ Update successful!");
            println!("   Result: {:#?}", result);
        }
        Err(e) => {
            println!("   ❌ Update failed: {}", e);
        }
    }

    // Check balance AFTER update
    println!("\n📊 Balance AFTER update:");
    match client.balance_allowance(Default::default()).await {
        Ok(balance) => {
            println!("   Balance: {}", balance.balance);
            println!("   Allowances: {:?}", balance.allowances);
        }
        Err(e) => {
            println!("   ❌ Failed: {}", e);
        }
    }

    Ok(())
}
