use anyhow::{Result, anyhow};
use async_trait::async_trait;
use std::collections::HashMap;
use tracing::{info, warn};
use crate::polymarket::{MarketInterface, MarketData};

/// Simulates market interactions for backtesting
pub struct MarketSimulator {
    // Current simulated time or tick index could be stored here
    active_markets: Vec<MarketData>,
    balance: f64,
    positions: HashMap<String, f64>, // (MarketID, SizeUSD)
    
    // Backtesting Fields
    historical_ticks: Vec<Tick>,
    current_tick_index: usize,
}

#[derive(Debug, Clone)]
pub struct Tick {
    pub timestamp: u64,
    pub market_id: String,
    pub price: f64,
    pub volume: f64,
    // We could add L2 updates here too
}

impl MarketSimulator {
    pub fn new() -> Self {
        Self {
            active_markets: Vec::new(),
            balance: 10_000.0, // Start with $10k paper money
            positions: HashMap::new(),
            historical_ticks: Vec::new(),
            current_tick_index: 0,
        }
    }
    
    /// Load mock data for testing
    pub fn load_markets(&mut self, markets: Vec<MarketData>) {
        self.active_markets = markets;
        info!("🎞️  Simulator loaded {} markets", self.active_markets.len());
    }

    /// Load historical ticks (Mock CSV loader for now)
    pub fn load_from_csv(&mut self, _path: &str) {
        // TODO: Implement actual CSV parsing
        // Creating mock ticks for demonstration
        self.historical_ticks = vec![
            Tick { timestamp: 1000, market_id: "mkt1".into(), price: 0.50, volume: 100.0 },
            Tick { timestamp: 2000, market_id: "mkt1".into(), price: 0.55, volume: 200.0 },
            Tick { timestamp: 3000, market_id: "mkt1".into(), price: 0.45, volume: 150.0 },
        ];
        info!("🎞️  Simulator loaded {} historical ticks", self.historical_ticks.len());
    }

    /// Advance simulation by one tick
    pub fn next_tick(&mut self) -> Option<&Tick> {
        if self.current_tick_index >= self.historical_ticks.len() {
            return None;
        }
        
        let tick = &self.historical_ticks[self.current_tick_index];
        self.current_tick_index += 1;
        
        // Update valid market state based on this tick
        if let Some(market) = self.active_markets.iter_mut().find(|m| m.id == tick.market_id) {
             // Simply update price for yes/no (assuming tick is YES price)
             market.yes_price = tick.price;
             market.no_price = 1.0 - tick.price;
             market.volume += tick.volume;
        }

        Some(tick)
    }
}

#[async_trait]
impl MarketInterface for MarketSimulator {
    async fn get_active_markets(&self) -> Result<Vec<MarketData>> {
        // In a real backtester, this would return the slice of markets valid at `current_time`
        // Should we auto-advance tick here? Or external control?
        // Ideally Agent calls this in loop. 
        // For simplicity, let's just return current state. The 'runner' drives the ticks.
        Ok(self.active_markets.clone())
    }

    async fn get_market_details(&self, market_id: &str) -> Result<MarketData> {
        self.active_markets
            .iter()
            .find(|m| m.id == market_id)
            .cloned()
            .ok_or_else(|| anyhow!("Market {} not found in simulation", market_id))
    }

    async fn get_balance(&self) -> Result<f64> {
        Ok(self.balance)
    }

    async fn place_order(
        &self,
        market_id: &str,
        side: &str,
        size: f64,
        price: f64,
    ) -> Result<String> {
        // Simulate immediate fill at the requested price
        info!("⚡ [SIM] Order Placed: {} {} @ ${:.2} on {}", side, size, price, market_id);
        
        // In a real simulator, we'd deduct balance, update positions, check liquidity etc.
        // For now, simple logging success.
        
        Ok(format!("sim-order-{}", uuid::Uuid::new_v4()))
    }
}

// Reuse UUID logic or import (duplication to avoid dep complexity for now, or just use uuid crate if added)
// Since helper mod uuid is private in client.rs, we use uuid crate directly if available, 
// or valid simple mock string.
mod uuid {
    use std::time::{SystemTime, UNIX_EPOCH};
    pub struct Uuid;
    impl Uuid {
        pub fn new_v4() -> String {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            format!("{:x}", now)
        }
    }
}
