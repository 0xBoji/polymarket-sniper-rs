use polymarket_client_sdk::{POLYGON, derive_proxy_wallet};
use polymarket_client_sdk::types::Address;
use alloy::signers::Signer;
use std::str::FromStr;

fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    
    let pk = std::env::var("POLYGON_PRIVATE_KEY")?;
    let signer = alloy::signers::local::LocalSigner::from_str(&pk)?
        .with_chain_id(Some(POLYGON));
    
    let eoa_address = signer.address();
    
    println!("🔍 Testing Wallet Derivation...");
    println!("   EOA Address: {}", eoa_address);
    println!();
    
    // Test Proxy wallet
    if let Some(proxy) = derive_proxy_wallet(eoa_address, POLYGON) {
        println!("📦 Proxy Wallet (Magic/Email):");
        println!("   SDK Derived: {}", proxy);
    }
    
    // Test Safe wallet
    if let Some(safe) = polymarket_client_sdk::derive_safe_wallet(eoa_address, POLYGON) {
        println!("\n🔐 Safe Wallet (Browser/MetaMask):");
        println!("   SDK Derived: {}", safe);
    }
    
    // Compare with env
    println!("\n📋 .env Configuration:");
    if let Ok(env_proxy) = std::env::var("POLYMARKET_PROXY_ADDRESS") {
        println!("   POLYMARKET_PROXY_ADDRESS: {}", env_proxy);
        
        let env_proxy_addr = Address::from_str(&env_proxy)?;
        if let Some(proxy) = derive_proxy_wallet(eoa_address, POLYGON) {
            if proxy == env_proxy_addr {
                println!("   ✅ Matches Proxy Wallet!");
            }
        }
        if let Some(safe) = polymarket_client_sdk::derive_safe_wallet(eoa_address, POLYGON) {
            if safe == env_proxy_addr {
                println!("   ✅ Matches Safe Wallet!");
            }
        }
    }
    
    Ok(())
}
