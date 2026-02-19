use anyhow::Result;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use polymarket_hft_agent::analytics::PnLTracker;
use polymarket_hft_agent::config::Config;
use polymarket_hft_agent::sniper::Sniper;

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

    // Create and run sniper
    let mut sniper = Sniper::new(config, pnl_tracker).await?;

    // Run sniper (this blocks until Ctrl+C)
    let sniper_result = sniper.run().await;

    // Cleanup

    sniper_result
}

fn print_banner(config: &Config) {
    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║          Polymarket HFT Agent with OpenRouter            ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
    println!();
    if config.predictive.enabled {
        println!("🚀 Strategy: Last-Minute Crypto Predictive (Binance + Polymarket)");
        println!("⏳ Final Window: {}s", config.predictive.final_window_sec);
        println!(
            "📈 Binance Threshold: {:.2}%",
            config.predictive.binance_signal_threshold_pct
        );
        println!(
            "💵 Max Entry Price: {:.2}",
            config.predictive.max_entry_price
        );
    } else if config.arbitrage.enabled {
        println!("🚀 Strategy: Intra-Market Arbitrage (Sniper Mode)");
        println!("💰 Min Edge: {} bps", config.arbitrage.min_edge_bps);
        println!(
            "💰 Max Size: ${:.2}",
            config.arbitrage.max_position_size_usd
        );
    } else {
        println!("🚀 Strategy: Expiration Sniping");
    }
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
    println!(
        "⏱️  Poll Interval: {} seconds",
        config.agent.market_poll_interval_secs
    );
    println!();
    println!("Press Ctrl+C to stop");
    println!("═══════════════════════════════════════════════════════════");
    println!();
}
