use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub id: String,
    pub question: String,
    pub end_date: Option<String>,
    pub volume: f64,
    pub liquidity: f64,
    pub yes_price: f64,
    pub no_price: f64,
    pub description: Option<String>,
    // L2 Data
    #[serde(default)]
    pub order_book_imbalance: f64,
    #[serde(default)]
    pub best_bid: f64,
    #[serde(default)]
    pub best_ask: f64,
    #[serde(default)]
    pub asset_ids: Vec<String>, // Token IDs for YES/NO
}

/// Represents a single price level in the orderbook
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct OrderLevel {
    pub price: f64,
    pub size: f64, // Size in USD or contracts
}

impl Default for OrderLevel {
    fn default() -> Self {
        Self {
            price: 0.0,
            size: 0.0,
        }
    }
}

/// Full L2 orderbook with depth
/// Uses fixed-size arrays for zero-allocation and cache locality
#[derive(Debug, Clone)]
pub struct OrderBook {
    pub bids: [OrderLevel; 50],
    pub asks: [OrderLevel; 50],
    pub bid_count: usize,
    pub ask_count: usize,
    pub timestamp: u64,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: [OrderLevel::default(); 50],
            asks: [OrderLevel::default(); 50],
            bid_count: 0,
            ask_count: 0,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
    
    #[inline(always)]
    pub fn best_bid(&self) -> Option<f64> {
        if self.bid_count > 0 {
            Some(self.bids[0].price)
        } else {
            None
        }
    }
    
    #[inline(always)]
    pub fn best_ask(&self) -> Option<f64> {
        if self.ask_count > 0 {
            Some(self.asks[0].price)
        } else {
            None
        }
    }
    
    #[inline(always)]
    pub fn total_bid_liquidity(&self) -> f64 {
        self.bids[..self.bid_count]
            .iter()
            .map(|level| level.size)
            .sum()
    }
    
    #[inline(always)]
    pub fn total_ask_liquidity(&self) -> f64 {
        self.asks[..self.ask_count]
            .iter()
            .map(|level| level.size)
            .sum()
    }
    
    /// Get active bid levels as slice
    #[inline(always)]
    pub fn bid_levels(&self) -> &[OrderLevel] {
        &self.bids[..self.bid_count]
    }
    
    /// Get active ask levels as slice
    #[inline(always)]
    pub fn ask_levels(&self) -> &[OrderLevel] {
        &self.asks[..self.ask_count]
    }
}
