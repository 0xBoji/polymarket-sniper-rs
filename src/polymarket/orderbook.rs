use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct Level2Quote {
    pub price: f64,
    pub size: f64,
}

#[derive(Debug, Clone)]
pub struct OrderBook {
    // Bids: Sorted High to Low (Reverse order for standard BTreeMap? No, BTreeMap is Low to High)
    // We want best bid (highest) easily accessibly.
    // In Rust BTreeMap, `iter().next_back()` gives the highest key.
    pub bids: BTreeMap<u64, f64>, // Price(bps) -> Size
    pub asks: BTreeMap<u64, f64>, // Price(bps) -> Size
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    /// Update a level
    pub fn update(&mut self, side: &str, price: f64, size: f64) {
        // Store price as basis points (u64) to use as key
        let price_bps = (price * 10000.0) as u64;
        
        if size == 0.0 {
            if side == "BUY" {
                self.bids.remove(&price_bps);
            } else {
                self.asks.remove(&price_bps);
            }
        } else {
            if side == "BUY" {
                self.bids.insert(price_bps, size);
            } else {
                self.asks.insert(price_bps, size);
            }
        }
    }

    /// Calculate Order Book Imbalance (OBI)
    /// (Total Bid Size - Total Ask Size) / (Total Bid Size + Total Ask Size)
    /// Closer to 1.0 means Strong Buying Pressure
    /// Closer to -1.0 means Strong Selling Pressure
    pub fn calculate_imbalance(&self) -> f64 {
        let total_bid_size: f64 = self.bids.values().sum();
        let total_ask_size: f64 = self.asks.values().sum();

        if total_bid_size + total_ask_size == 0.0 {
            return 0.0;
        }

        (total_bid_size - total_ask_size) / (total_bid_size + total_ask_size)
    }

    /// Get Best Bid and Best Ask
    pub fn best_quote(&self) -> (Option<f64>, Option<f64>) {
        let best_bid = self.bids.keys().next_back().map(|&p| p as f64 / 10000.0); // Highest key
        let best_ask = self.asks.keys().next().map(|&p| p as f64 / 10000.0);      // Lowest key
        (best_bid, best_ask)
    }
}
