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
