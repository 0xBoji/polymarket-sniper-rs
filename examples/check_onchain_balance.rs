use alloy::primitives::{Address, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::sol;
use std::str::FromStr;

// USDC contract ABI
sol! {
    #[sol(rpc)]
    contract USDC {
        function balanceOf(address account) external view returns (uint256);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let rpc_url = std::env::var("POLYGON_WS_RPC")?
        .replace("wss://", "https://")
        .replace("/v2/", "/v2/");

    let provider = ProviderBuilder::new().on_http(rpc_url.parse()?);

    // USDC contract on Polygon
    let usdc_address = Address::from_str("0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359")?;
    let proxy_wallet = Address::from_str("0x5e1C883545b4325Ae9348E57edE2deC85789f54b")?;

    let usdc = USDC::new(usdc_address, &provider);

    println!("🔍 Checking USDC balance on-chain...");
    println!("   Proxy Wallet: {}", proxy_wallet);

    match usdc.balanceOf(proxy_wallet).call().await {
        Ok(balance) => {
            let balance_f64 = balance._0.to_string().parse::<f64>().unwrap_or(0.0) / 1_000_000.0;
            println!("   ✅ On-chain USDC Balance: ${:.2}", balance_f64);
        }
        Err(e) => {
            println!("   ❌ Failed to check balance: {}", e);
        }
    }

    Ok(())
}
