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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderLevel {
    pub price: f64,
    pub size: f64, // Size in USD or contracts
}

/// Full L2 orderbook with depth
#[derive(Debug, Clone)]
pub struct OrderBook {
    pub bids: Vec<OrderLevel>,
    pub asks: Vec<OrderLevel>,
    pub timestamp: u64,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: Vec::new(),
            asks: Vec::new(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
    
    pub fn best_bid(&self) -> Option<f64> {
        self.bids.first().map(|level| level.price)
    }
    
    pub fn best_ask(&self) -> Option<f64> {
        self.asks.first().map(|level| level.price)
    }
    
    pub fn total_bid_liquidity(&self) -> f64 {
        self.bids.iter().map(|level| level.size).sum()
    }
    
    pub fn total_ask_liquidity(&self) -> f64 {
        self.asks.iter().map(|level| level.size).sum()
    }
}
