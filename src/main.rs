use anyhow::Result;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tracing::{info, warn};
use std::sync::{Arc, Mutex};

use polymarket_hft_agent::sniper::Sniper;
use polymarket_hft_agent::config::Config;
use polymarket_hft_agent::analytics::{self, PnLTracker};

// Unused imports removed



#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "polymarket_hft_agent=debug,info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = Config::from_env()?;
    
    // Phase 2 Optimization: CPU Pinning
    // Pin main thread to dedicated core for consistent latency
    if let Some(pinner) = polymarket_hft_agent::execution::CpuPinner::new() {
        info!("🎯 CPU cores available: {}", pinner.core_count());
        if pinner.pin_strategy_thread() {
            info!("✅ Strategy thread pinned to core 0");
        } else {
            warn!("⚠️ Could not pin strategy thread");
        }
    } else {
        warn!("⚠️ CPU pinning not available on this system");
    }

    // Print startup banner
    print_banner(&config);

    // Initialize PnL tracker
    let pnl_tracker = Arc::new(Mutex::new(PnLTracker::new(1000.0))); // $1000 initial capital
    
    // Small delay to ensure tokio runtime is fully initialized
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // TEST: Bind in main thread to verify network
    println!("DEBUG: [MAIN] Attempting bind to 3002 in main thread...");
    {
        let listener = std::net::TcpListener::bind("0.0.0.0:3002");
        match listener {
            Ok(_) => println!("DEBUG: [MAIN] ✅ Sync bind to 3002 SUCCESS! (Dropping now)"),
            Err(ref e) => println!("DEBUG: [MAIN] ❌ Sync bind to 3002 FAILED: {}", e),
        }
    } // Listener drops here automatically
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Spawn API server
    let tracker_clone = pnl_tracker.clone();
    println!("DEBUG: [MAIN] Spawning dashboard task...");
    let server_handle = tokio::spawn(async move {
        // ... (existing code, maybe switch back to original run_server later)
        println!("DEBUG: [TASK] Dashboard task started!");
         analytics::api::run_server(tracker_clone).await;
         println!("DEBUG: [TASK] Dashboard task ENDED");
    });
    
    // Give server a moment to start
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

    // Self-test dashboard
    println!("DEBUG: [MAIN] performing self-test on port 3002...");
    match reqwest::get("http://127.0.0.1:3002/api/stats").await {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("DEBUG: [MAIN] ✅ SELF-TEST PASSED on port 3002");
                tracing::info!("✅ SELF-TEST: Dashboard server is RESPONDING on port 3002");
            } else {
                println!("DEBUG: [MAIN] ❌ SELF-TEST Failed status: {}", resp.status());
                tracing::error!("❌ SELF-TEST: Dashboard server returned status {}", resp.status());
            }
        }
        Err(e) => {
             println!("DEBUG: [MAIN] ❌ SELF-TEST ERROR: {}", e);
             tracing::error!("❌ SELF-TEST: Failed to connect to dashboard: {}", e);
        }
    }

    // Create and run sniper
    let mut sniper = Sniper::new(config, pnl_tracker).await;
    
    // Run sniper (this blocks until Ctrl+C)
    let sniper_result = sniper.run().await;
    
    // Cleanup
    server_handle.abort();
    
    sniper_result
}

fn print_banner(config: &Config) {
    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║          Polymarket HFT Agent with OpenRouter            ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
    println!();
    println!("🚀 Strategy: Intra-Market Arbitrage (Sniper Mode)");
    println!("💰 Min Edge: {} bps", config.arbitrage.min_edge_bps);
    println!("💰 Max Size: ${:.2}", config.arbitrage.max_position_size_usd);
    println!(
        "📊 Mode: {}",
        if config.agent.paper_trading {
            "PAPER TRADING (Safe Mode)"
        } else {
            "⚠️  LIVE TRADING ⚠️"
        }
    );
    println!("📊 Risk Settings:");
    println!(
        "   • Max Position: {:.1}% of capital",
        config.risk.max_position_size_pct
    );
    println!(
        "   • Max Portfolio Exposure: {:.1}%",
        config.risk.max_portfolio_exposure_pct
    );
    println!("   • Stop Loss: {:.1}%", config.risk.stop_loss_pct);
    println!("🔍 Market Filters:");
    println!(
        "   • Min Volume: ${:.0}",
        config.market_filters.min_market_volume
    );
    println!(
        "   • Min Liquidity: ${:.0}",
        config.market_filters.min_liquidity
    );
    println!("⏱️  Poll Interval: {} seconds", config.agent.market_poll_interval_secs);
    println!();
    println!("Press Ctrl+C to stop");
    println!("═══════════════════════════════════════════════════════════");
    println!();
}
