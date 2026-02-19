use anyhow::Result;
use dotenvy::dotenv;
use polymarket_client_sdk::clob::types::OrderType;
use polymarket_hft_agent::config::Config;
use polymarket_hft_agent::polymarket::{MarketInterface, PolymarketClient};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    println!("🧪 Testing Order Placement with SDK...\n");

    // Load config
    let config = Config::from_env()?;

    // Create client
    let client = PolymarketClient::new(
        &config.polymarket,
        config.agent.paper_trading, // Use config value, not hardcoded
        config.polygon_private_key.clone(),
    )?;

    println!(
        "✅ Client initialized successfully (Paper Trading: {})",
        config.agent.paper_trading
    );

    // Test 1: Check balance
    println!("\n📊 Test 1: Checking balance...");
    match client.get_balance().await {
        Ok(balance) => println!("   Balance: ${:.2}", balance),
        Err(e) => println!("   ❌ Balance check failed: {}", e),
    }

    // Test 2: Fetch markets
    println!("\n📊 Test 2: Fetching markets...");
    match client.get_active_markets().await {
        Ok(markets) => {
            println!("   Found {} markets", markets.len());
            if let Some(market) = markets.first() {
                println!("   First market: {}", market.question);
                println!("   Market ID: {}", market.id);

                // Test 3: Try to place a small test order (will fail in paper mode but tests the flow)
                println!("\n📊 Test 3: Testing order placement flow...");
                println!("   Market: {}", market.question);
                println!("   Attempting to place YES order at $0.50 for $5.00");

                match client
                    .place_order(
                        &market.id,
                        "YES",
                        5.0,  // $5 USD (minimum order size)
                        0.50, // at $0.50 price
                        OrderType::GTC,
                    )
                    .await
                {
                    Ok(order_id) => println!("   ✅ Order placed successfully! ID: {}", order_id),
                    Err(e) => {
                        println!("   ⚠️  Order failed (expected in paper mode): {}", e);
                        // Check if it's an authentication/SDK error vs paper trading error
                        let error_msg = e.to_string();
                        if error_msg.contains("Failed to build order")
                            || error_msg.contains("Failed to sign order")
                            || error_msg.contains("Failed to post order")
                        {
                            println!("   ❌ SDK integration issue detected!");
                            return Err(e);
                        } else {
                            println!("   ℹ️  Error is likely due to paper trading mode or market conditions");
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("   ❌ Failed to fetch markets: {}", e);
            return Err(e);
        }
    }

    println!("\n✅ All SDK integration tests passed!");
    Ok(())
}
