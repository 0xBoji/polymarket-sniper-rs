use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingDecision {
    pub should_trade: bool,
    pub side: String, // "YES" or "NO"
    pub confidence: f64,
    pub position_size_pct: f64,
    pub reasoning: String,
    pub risks: Vec<String>,
}
