use anyhow::Result;
use tokio::time::{interval, Duration};
use tokio::sync::mpsc;
use tracing::{error, info, warn, debug};
use std::sync::{Arc, Mutex};
use ethers::types::Address;

use crate::config::Config;
use crate::strategies::risk::RiskManager;
use crate::strategies::arbitrage::{ArbitrageStrategy, TradeAction};
use crate::execution::{Executor, RedemptionManager};
use crate::polymarket::{MarketData, MempoolMonitor, PolymarketClient, MarketInterface, MarketEventListener};
use crate::simulation::MarketSimulator;
use crate::analytics::{PnLTracker, pnl::Position};
use std::collections::{HashSet, HashMap};
use std::collections::VecDeque;
use chrono::Utc;
use crate::polymarket::ws::{ClobWebSocket, OrderbookUpdate};

pub struct Sniper {
    config: Config,
    market_interface: Box<dyn MarketInterface>,
    risk_manager: RiskManager,
    _start_time: chrono::DateTime<chrono::Utc>, // Keep track of uptime
    strategy: ArbitrageStrategy,
    executor: Executor,
    _mempool_monitor: MempoolMonitor,
    redemption_manager: Option<RedemptionManager>,
    seen_markets: HashSet<String>,
    pnl_tracker: Arc<Mutex<PnLTracker>>,
    new_market_rx: Option<mpsc::UnboundedReceiver<String>>, // From WebSocket events
    pending_retries: VecDeque<(String, u8)>, // (MarketID, RetryCount)
    // WebSocket CLOB
    ws_client: Option<ClobWebSocket>,
    ws_update_rx: Option<mpsc::Receiver<OrderbookUpdate>>,
    active_markets: HashMap<String, MarketData>,
    asset_map: HashMap<String, (String, String)>, // AssetID -> (MarketID, Side)
}

impl Sniper {
    pub async fn new(config: Config, pnl_tracker: Arc<Mutex<PnLTracker>>) -> Self {
        // Initialize Market Interface (Real or Sim)
        let market_interface: Box<dyn MarketInterface> = if config.agent.simulation_mode {
            info!("🎞️  Initializing Market Simulator");
            Box::new(MarketSimulator::new())
        } else {
            info!("🌐 Initializing Real Polymarket Client");
            Box::new(PolymarketClient::new(
                &config.polymarket, 
                config.agent.paper_trading
            ))
        };

        let risk_manager = RiskManager::new(config.risk.clone());
        let strategy = ArbitrageStrategy::new(config.arbitrage.clone());
        
        // Executor needs a separate instance or clone.
        // For Sim: new simulator instance (Note: State sharing logic needed if we want positions to sync)
        // Ideally we pass Arc<Mutex<Simulator>>.
        // For now, creating a fresh simulator means the executor has its own empty state. 
        // This is a known limitation of current refactor (Step 7 limitation).
        // TODO: Use Arc<dyn MarketInterface> for shared state in future.
        let executor_interface: Box<dyn MarketInterface> = if config.agent.simulation_mode {
            Box::new(MarketSimulator::new()) 
        } else {
            Box::new(PolymarketClient::new(&config.polymarket, config.agent.paper_trading))
        };

        let executor = Executor::new(executor_interface);
        
        let mempool_monitor = MempoolMonitor::new(config.polygon_ws_rpc.clone()).await;

        let redemption_manager = if let (Some(rpc), Some(pk)) = (
            &config.polygon_ws_rpc,
            &config.polygon_private_key,
        ) {
            match RedemptionManager::new(rpc, pk).await {
                Ok(rm) => {
                    info!("✅ RedemptionManager initialized");
                    Some(rm)
                }
                Err(e) => {
                    error!("❌ Failed to init RedemptionManager: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Setup WebSocket event listener if configured
        let new_market_rx = if let (Some(ws_url), Some(ctf_addr_str)) = (
            &config.polygon_ws_rpc,
            &config.ctf_contract_address,
        ) {
            // Parse CTF contract address
            if let Ok(ctf_address) = ctf_addr_str.parse::<Address>() {
                let (tx, rx) = mpsc::unbounded_channel();
                
                // Spawn WebSocket listener task
                let ws_url_clone = ws_url.clone();
                tokio::spawn(async move {
                    match MarketEventListener::new(&ws_url_clone, ctf_address).await {
                        Ok(listener) => {
                            if let Err(e) = listener.listen_for_new_markets(tx).await {
                                error!("❌ WebSocket listener error: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("❌ Failed to create WebSocket listener: {}", e);
                        }
                    }
                });
                
                info!("⚡ WebSocket real-time detection enabled");
                Some(rx)
            } else {
                warn!("⚠️ Invalid CTF contract address, WebSocket disabled");
                None
            }
        } else {
            info!("📊 WebSocket disabled, using polling only");
            None
        };

        // Initialize CLOB WebSocket
        let (ws_client, ws_update_rx) = if !config.agent.simulation_mode {
            let (tx, rx) = mpsc::channel(1000);
            match ClobWebSocket::new(tx).await {
                Ok(ws) => (Some(ws), Some(rx)),
                Err(e) => {
                    error!("❌ Failed to init CLOB WS: {}", e);
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

        Self {
            config,
            market_interface,
            risk_manager,
            _start_time: Utc::now(),
            strategy,
            executor,
            _mempool_monitor: mempool_monitor,
            redemption_manager,
            seen_markets: HashSet::new(),
            pnl_tracker,
            new_market_rx,
            pending_retries: VecDeque::new(),
            ws_client,
            ws_update_rx,
            active_markets: HashMap::new(),
            asset_map: HashMap::new(),
        }
    }

    /// Main agent loop
    pub async fn run(&mut self) -> Result<()> {
        info!("🚀 Starting Polymarket HFT Agent");
        
        // Start background tasks
        // self.mempool_monitor.start_monitoring().await;
        info!("⚠️ Mempool monitoring disabled to save API credits");

        info!(
            "📊 Mode: {}",
            if self.config.agent.paper_trading {
                "PAPER TRADING"
            } else {
                "LIVE TRADING"
            }
        );
        info!(
            "⏱️  Poll interval: {} seconds",
            self.config.agent.market_poll_interval_secs
        );

        let mut tick_interval = interval(Duration::from_secs(
            self.config.agent.market_poll_interval_secs,
        ));

        // Redemption check interval (every 5 minutes)
        let mut redemption_interval = interval(Duration::from_secs(300));
        
        // PnL update interval (every 10 seconds)
        let mut pnl_update_interval = interval(Duration::from_secs(10));

        // Retry interval (every 1 second)
        let mut retry_interval = interval(Duration::from_secs(1));

        loop {
            tokio::select! {
                // 1. CLOB Orderbook Updates (HIGHEST PRIORITY)
                Some(update) = async {
                    match &mut self.ws_update_rx {
                        Some(rx) => rx.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                     // 1. Identify Market
                     if let Some((market_id, side)) = self.asset_map.get(&update.asset_id).cloned() {
                         debug!("⚡ Tick: {} ({} bids, {} asks)", side, update.bids.len(), update.asks.len());
                         
                         // 2. Update State
                         if let Some(market) = self.active_markets.get_mut(&market_id) {
                             // Update Prices based on Bids/Asks
                             // NOTE: We are SNIPING, so we want to BUY.
                             // Buying YES means taking the Lowest ASK.
                             // Buying NO means taking the Lowest ASK.
                             // So we care about ASKS.
                             
                             if let Some(best_ask) = update.asks.first() {
                                 let price = best_ask.price.parse::<f64>().unwrap_or(0.0);
                                 if side == "YES" {
                                     market.yes_price = price;
                                 } else {
                                     market.no_price = price;
                                 }
                                 
                                 // Trigger re-eval
                                 // Clone to avoid borrow issues while calling async func
                                 let market_clone = market.clone(); 
                                 
                                 // We need to spawn this or call it?
                                 // Calling await here might block other updates slightly, but it's the core logic.
                                 // For HFT, we should check strategy synchronously if possible, but execute async.
                                 // Our current process_single_market is async.
                                 // Let's call it.
                                 if let Err(e) = self.process_single_market(&market_clone).await {
                                     error!("❌ Processing error: {}", e);
                                 }
                             }
                         }
                     }
                }

                // WebSocket events (New Markets)
                Some(condition_id) = async {
                    match &mut self.new_market_rx {
                        Some(rx) => rx.recv().await,
                        None => std::future::pending().await, // Never resolves if no WS
                    }
                } => {
                    info!("⚡ WebSocket event: New market condition {}", condition_id);

                    // OPTIMIZATION: Check if we've already seen this market (e.g. via polling or previous event)
                    if self.seen_markets.contains(&condition_id) {
                        continue;
                    }
                    
                    // Try fetch ONCE immediately
                    // If it fails, add to retry queue to avoid blocking
                    info!("⚡ Triggering immediate fast market fetch...");
                    match self.market_interface.get_market_details(&condition_id).await {
                        Ok(market) => {
                            info!("✅ Fast sync success: {}", market.question);
                            if !self.seen_markets.contains(&market.id) {
                                self.seen_markets.insert(market.id.clone());
                                info!("🚀 Processing new market immediately: {}", market.question);
                                if let Err(e) = self.process_single_market(&market).await {
                                    error!("❌ Error processing market {}: {}", market.question, e);
                                }
                            }
                        },
                        Err(e) => {
                            // Non-blocking retry: Queue it
                            warn!("⚠️ Initial fetch failed ({}), queuing for retry...", e);
                            self.pending_retries.push_back((condition_id, 1));
                        }
                    }
                }

                // Generic Retry Processing (Non-blocking)
                _ = retry_interval.tick() => {
                    let mut still_pending = VecDeque::new();
                    let max_attempts = 10; // Allow more attempts since non-blocking

                    while let Some((condition_id, attempts)) = self.pending_retries.pop_front() {
                        // Check if seen in meantime
                        if self.seen_markets.contains(&condition_id) {
                            continue;
                        }

                        // Try fetch
                        match self.market_interface.get_market_details(&condition_id).await {
                            Ok(market) => {
                                info!("✅ Retry success for {} after {} attempts", market.question, attempts);
                                if !self.seen_markets.contains(&market.id) {
                                    self.seen_markets.insert(market.id.clone());
                                    info!("🚀 Processing queued market: {}", market.question);
                                    if let Err(e) = self.process_single_market(&market).await {
                                        error!("❌ Error processing market {}: {}", market.question, e);
                                    }
                                }
                            },
                            Err(_) => {
                                if attempts < max_attempts {
                                    // Push back to end of NEW queue (round robin style if multiple)
                                    still_pending.push_back((condition_id, attempts + 1));
                                } else {
                                    error!("❌ Gave up fetching {} after {} attempts", condition_id, max_attempts);
                                    // Trigger fallback full sync ONLY when giving up? 
                                    // Or just let polling catch it eventually.
                                }
                            }
                        }
                    }
                    
                    // Restore pending
                    self.pending_retries = still_pending;
                }
                
                // Polling (BACKUP - catches anything WS might miss)
                _ = tick_interval.tick() => {
                    if let Err(e) = self.process_markets().await {
                        error!("❌ Error processing markets: {}", e);
                    }
                }
                
                _ = redemption_interval.tick() => {
                    if let Some(rm) = &self.redemption_manager {
                         // Iterate all positions and check if resolved
                         // Optimization: in real app, maintain a list of 'potential to redeem'
                         let positions = self.risk_manager.get_positions();
                         for pos in positions {
                             match rm.is_condition_resolved(&pos.market_id).await {
                                 Ok(resolved) => {
                                     if resolved {
                                         info!("🎉 Market {} resolved! Redeeming...", pos.market_id);
                                         if let Err(e) = rm.redeem_positions(&pos.market_id).await {
                                            error!("❌ Redemption failed for {}: {}", pos.market_id, e);
                                         } else {
                                             // Remove position from risk manager upon successful redemption request
                                             // (Or wait for confirmation, but for now remove to free up exposure)
                                             self.risk_manager.remove_position(&pos.market_id);
                                         }
                                     }
                                 }
                                 Err(e) => {
                                     warn!("⚠️ Failed to check resolution for {}: {}", pos.market_id, e);
                                 }
                             }
                         }
                    }
                }
                _ = pnl_update_interval.tick() => {
                    // Update prices with LIVE data
                    let mut market_ids: Vec<String> = Vec::new();
                    if let Ok(tracker) = self.pnl_tracker.lock() {
                        // Get unique market IDs from active positions
                        for pos in tracker.positions.values() {
                            if !market_ids.contains(&pos.market_id) {
                                market_ids.push(pos.market_id.clone());
                            }
                        }
                    }

                    // Fetch updates for these markets
                    if !market_ids.is_empty() {
                        // info!("Updating PnL for {} markets...", market_ids.len());
                        for market_id in market_ids {
                            match self.market_interface.get_market_details(&market_id).await {
                                Ok(market) => {
                                    if let Ok(mut tracker) = self.pnl_tracker.lock() {
                                        tracker.update_market_price(&market_id, market.yes_price, market.no_price);
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to fetch price for PnL update {}: {}", market_id, e);
                                }
                            }
                        }
                    }

                    // Take snapshot after updates
                    if let Ok(mut tracker) = self.pnl_tracker.lock() {
                        tracker.take_snapshot();
                    }
                }
            }
        }
    }

    /// Process new markets and manage positions
    async fn process_markets(&mut self) -> Result<()> {
        // Fetch current state of all markets
        let all_markets = self.market_interface.get_active_markets().await?;

        // FIRST TIME ONLY: Mark all existing markets as seen without analyzing
        // This prevents analyzing 1000+ old markets on startup
        if self.seen_markets.is_empty() {
            for market in &all_markets {
                self.seen_markets.insert(market.id.clone());
            }
            info!("🚀 Startup: Skipped {} existing markets (only trading NEW markets from now on)", all_markets.len());
            
            // Still manage positions even on first run
            self.manage_positions(&all_markets).await?;
            return Ok(());
        }

        // SUBSEQUENT RUNS: Detect and process NEW markets only
        let mut new_markets = Vec::new();
        for market in &all_markets {
            if !self.seen_markets.contains(&market.id) {
                info!("🆕 NEW market detected: {}", market.question);
                self.seen_markets.insert(market.id.clone());
                new_markets.push(market.clone());
            }
        }

        if !new_markets.is_empty() {
            info!("⚡ Processing {} brand new markets", new_markets.len());
            for market in new_markets {
                if let Err(e) = self.process_single_market(&market).await {
                    error!("❌ Error processing market {}: {}", market.question, e);
                    continue;
                }
            }
        }

        // Manage existing positions
        self.manage_positions(&all_markets).await?;

        Ok(())
    }

    /// Process a single market through the entire pipeline
    async fn process_single_market(&mut self, market: &MarketData) -> Result<()> {
        // Filter out garbage markets
        if !self.passes_filters(market) {
            return Ok(());
        }

        // Register for WS Updates
        if let Some(ws) = &self.ws_client {
             if !market.asset_ids.is_empty() {
                 info!("🔌 Subscribing to Orderbook for {}", market.question);
                 ws.subscribe(market.asset_ids.clone());
                 
                 // Cache state
                 self.active_markets.insert(market.id.clone(), market.clone());
                 
                 // Map Asset Maps
                 // Assuming asset_ids[0] = NO, asset_ids[1] = YES (This is standard for Polymarket CTF)
                 // But wait, let's just Map them both. We need to know which is which.
                 // In `client.rs` we didn't strictly order them, but `outcome_prices` logic assumed order.
                 // IMPORTANT: We need to know which asset_id is YES and which is NO.
                 // For now, let's simplistically Map: 
                 // If we have 2 assets, assign them based on index (Hack for now, need robust mapping later)
                 if market.asset_ids.len() >= 2 {
                     self.asset_map.insert(market.asset_ids[0].clone(), (market.id.clone(), "NO".to_string()));
                     self.asset_map.insert(market.asset_ids[1].clone(), (market.id.clone(), "YES".to_string()));
                 }
             }
        }

        // 1. Check Strategy (Sniper Mode: Intra-Market Arbitrage)
        match self.strategy.check_opportunity(market) {
            TradeAction::BuyBoth { market_id: _, yes_price, no_price, size_usd, expected_profit_bps } => {
                info!("🎯 Sniper Signal: {} (Profit: {} bps)", market.question, expected_profit_bps);

                // 2. Risk Check
                // Note: For arb, confidence is essentially 100% (1.0) if we trust the orderbook
                if self.risk_manager.validate_entry(&market.id, size_usd, 1.0) {
                     info!("⚡ Executing ARBITRAGE for {}", market.question);
                     
                     let trade_id = format!("arb_{}_{}", market.id, Utc::now().timestamp_millis());

                     // Execute YES
                     let decision_yes = crate::strategies::types::TradingDecision {
                         should_trade: true,
                         side: "YES".to_string(),
                         confidence: 1.0,
                         position_size_pct: 0.1, // Fixed for now, handled by size_usd in reality
                         reasoning: "Arbitrage".to_string(),
                         risks: vec![],
                     };
                     
                     // Execute NO
                     let decision_no = crate::strategies::types::TradingDecision {
                         should_trade: true,
                         side: "NO".to_string(),
                         confidence: 1.0,
                         position_size_pct: 0.1,
                         reasoning: "Arbitrage".to_string(),
                         risks: vec![],
                     };

                     // Execute YES
                     if let Err(e) = self.executor.execute_trade(&decision_yes, market, &trade_id, &mut self.risk_manager).await {
                         error!("❌ Arb Execution Failed (YES): {}", e);
                     }
                     
                     // Execute NO
                     if let Err(e) = self.executor.execute_trade(&decision_no, market, &trade_id, &mut self.risk_manager).await {
                         error!("❌ Arb Execution Failed (NO): {}", e);
                     }
                     
                     // Update PnL (Mocking generic position for now)
                     let position = Position {
                        id: trade_id.clone(),
                        market_id: market.id.clone(),
                        market_question: market.question.clone(),
                        side: "BOTH".to_string(),
                        size: size_usd, // Total size roughly
                        entry_price: yes_price + no_price, // Arbitrage cost
                        current_price: yes_price + no_price,
                        entry_time: Utc::now(),
                    };
                    
                    if let Ok(mut tracker) = self.pnl_tracker.lock() {
                        tracker.add_position(position);
                    }
                }
            }
            TradeAction::None => {
                // info!("💤 No arb opportunity for {}", market.question);
            }
        }
        
        Ok(())
    }

    /// Manage active positions (Stop Loss, Take Profit)
    async fn manage_positions(&mut self, current_markets: &[MarketData]) -> Result<()> {
        let positions = self.risk_manager.get_positions();
        
        if positions.is_empty() {
            return Ok(());
        }

        // info!("🔄 Managing {} active positions", positions.len());

        for position in positions {
            // Find current market data
            if let Some(market) = current_markets.iter().find(|m| m.id == position.market_id) {
                let current_price = if position.side == "YES" {
                    market.yes_price
                } else {
                    market.no_price
                };

                // Check Stop Loss via RiskManager
                if self.risk_manager.check_stop_loss(&position, current_price) {
                    info!("🛑 Executing STOP LOSS for {}", market.question);
                    if let Err(e) = self.executor.close_position(market, &position.side, &mut self.risk_manager).await {
                        error!("❌ Failed to close position for {}: {}", market.question, e);
                    } else {
                        // Success: Update PnL Tracker
                        if let Ok(mut tracker) = self.pnl_tracker.lock() {
                            tracker.close_position(&position.trade_id);
                        }
                    }
                }

                // TODO: Add Take Profit logic here
            }
        }
        
        Ok(())
    }

    /// Check if market passes filters
    fn passes_filters(&self, market: &MarketData) -> bool {
        if market.volume < self.config.market_filters.min_market_volume {
            info!(
                "⏭️  Volume ${:.2} below minimum ${:.2}",
                market.volume, self.config.market_filters.min_market_volume
            );
            return false;
        }

        if market.liquidity < self.config.market_filters.min_liquidity {
            info!(
                "⏭️  Liquidity ${:.2} below minimum ${:.2}",
                market.liquidity, self.config.market_filters.min_liquidity
            );
            return false;
        }

        true
    }
}
