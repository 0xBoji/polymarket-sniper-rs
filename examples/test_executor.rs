use std::sync::Arc;
use polymarket_hft_agent::config::Config;
use polymarket_hft_agent::polymarket::{PolymarketClient, MarketInterface};
use polymarket_hft_agent::execution::Executor;
use polymarket_hft_agent::strategies::risk::RiskManager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Load configuration from .env
    let config = Config::from_env()?;
    
    println!("🔧 Initializing Test Executor...");
    println!("📊 Paper Trading: {}", config.agent.paper_trading);

    // 2. Initialize Polymarket Client
    let client = PolymarketClient::new(
        &config.polymarket,
        config.agent.paper_trading,
        config.polygon_private_key.clone(),
    );

    // 3. Check balances (Bridged vs Native)
    let bridged_usdc = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174";
    let native_usdc = "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359";
    
    // Using simple provider call for test
    use ethers::prelude::*;
    use std::str::FromStr;
    // CRITICAL FIX: Use HTTP RPC for one-off balance checks. WSS config causes BadScheme error with Provider::<Http>
    let provider = Provider::<Http>::try_from("https://polygon-rpc.com")?;
    
    // Choose address to check: Proxy if available, otherwise Signer
    let signer_address = LocalWallet::from_str(&config.polygon_private_key.clone().unwrap())?.address();
    let target_address = if let Some(proxy) = &config.polymarket.proxy_address {
        let proxy_addr = Address::from_str(proxy)?;
        println!("🛡️ Using Proxy Wallet for Balance Check: {:?}", proxy_addr);
        proxy_addr
    } else {
        println!("🕵️ Using Signer Wallet for Balance Check: {:?}", signer_address);
        signer_address
    };
    
    // Manual balance check via ERC20 interface
    abigen!(ERC20, r#"[function balanceOf(address account) external view returns (uint256)]"#);
    
    let bridged_contract = ERC20::new(Address::from_str(bridged_usdc).unwrap(), Arc::new(provider.clone()));
    let native_contract = ERC20::new(Address::from_str(native_usdc).unwrap(), Arc::new(provider.clone()));
    
    // STRICT RPC CHECKS (Panic if network fails)
    let b_balance: U256 = bridged_contract.balance_of(target_address).call().await.expect("❌ RPC Error: Failed to check Bridged USDC");
    let n_balance: U256 = native_contract.balance_of(target_address).call().await.expect("❌ RPC Error: Failed to check Native USDC");
    
    let b_f64 = b_balance.as_u128() as f64 / 1_000_000.0;
    let n_f64 = n_balance.as_u128() as f64 / 1_000_000.0;
    
    println!("💰 Bridged USDC (USDC.e): ${:.2}", b_f64);
    println!("💰 Native USDC (USDC):   ${:.2}", n_f64);

    // API BALANCE CHECK
    println!("🔍 Verifying via Polymarket API...");
    let api_balance = client.get_balance().await.unwrap_or_else(|e| {
        println!("❌ API Balance Check Failed: {}", e);
        0.0
    });
    println!("💰 API Reported Balance: ${:.2}", api_balance);

    if b_f64 < 2.0 && n_f64 < 2.0 && api_balance < 2.0 {
        println!("⚠️ WARNING: Insufficient balance for test (Need at least $2.0). Proceeding anyway to test AUTH and API...");
        // anyhow::bail!("Insufficient balance for test (Need at least $2.0).");
    }

    // 4. Fetch active markets and find a Bitcoin target
    println!("🔍 Fetching active markets...");
    let markets = client.get_active_markets().await?;
    
    let market = markets.iter()
        .find(|m| m.question.to_lowercase().contains("bitcoin") && m.yes_price > 0.05 && m.yes_price < 0.95)
        .or_else(|| markets.iter().find(|m| m.yes_price > 0.01 && m.yes_price < 0.99))
        .ok_or_else(|| anyhow::anyhow!("No suitable market found"))?;

    println!("🎯 Selected Market: {}", market.question);
    println!("🆔 Market ID: {}", market.id);
    println!("💰 YES Price: ${:.4} | NO Price: ${:.4}", market.yes_price, market.no_price);

    // 5. Initialize Executor
    let executor = Executor::new(Box::new(client), None);
    let mut risk_manager = RiskManager::new(config.risk.clone());

    // 6. Place a test order ($2.0 - ensures we are well above $1.0 min)
    let size_usd = 2.0;
    let price = market.yes_price; 
    let trade_id = format!("manual_test_{}", chrono::Utc::now().timestamp());

    println!("🚀 Placing order: ${:.2} on YES at ${:.4}...", size_usd, price);
    
    match executor.execute_snipe(
        market,
        "YES",
        price,
        size_usd,
        &trade_id,
        &mut risk_manager
    ).await {
        Ok(order_id) => {
            println!("✅ EXECUTION SUCCESS!");
            println!("🆔 Order ID: {}", order_id);
            println!("📝 Note: This was a real order (if PAPER_TRADING=false).");
        },
        Err(e) => {
            println!("❌ EXECUTION FAILED: {}", e);
            if e.to_string().contains("insufficient") {
                println!("💡 Tip: Check if you have enough USDC and MATIC/POL in your wallet.");
            }
        }
    }

    Ok(())
}
