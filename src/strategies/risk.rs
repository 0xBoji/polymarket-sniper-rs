use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

use crate::config::RiskConfig;
use crate::strategies::types::TradingDecision;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub market_id: String,
    pub trade_id: String, // Track specific trade for learning
    pub side: String,
    pub size_usd: f64,
    pub entry_price: f64,
    pub timestamp: u64,
}

pub struct RiskManager {
    config: RiskConfig,
    positions: HashMap<String, Position>, // market_id -> Position
}

impl RiskManager {
    pub fn new(config: RiskConfig) -> Self {
        Self {
            config,
            positions: HashMap::new(),
        }
    }

    /// Check if we should enter a trade based on risk limits
    pub fn validate_entry(&self, market_id: &str, size_usd: f64, confidence: f64) -> bool {
        // 1. Check duplicate position
        if self.positions.contains_key(market_id) {
            warn!("⚠️ Risk: Position already exists for market {}", market_id);
            return false;
        }

        // 2. Check position limit
        if size_usd > self.max_position_size() {
            warn!(
                "⚠️ Risk: Position size ${} exceeds limit ${}",
                size_usd,
                self.max_position_size()
            );
            return false;
        }

        // 3. Check portfolio exposure
        let current_exposure: f64 = self.positions.values().map(|p| p.size_usd).sum();
        let max_exposure = self.config.max_portfolio_exposure_pct * 1000.0; // Assuming $1000 capital for now
        if current_exposure + size_usd > max_exposure {
            warn!(
                "⚠️ Risk: Exposure ${} would exceed limit ${}",
                current_exposure + size_usd,
                max_exposure
            );
            return false;
        }

        // 4. Check confidence threshold
        if confidence < 0.6 {
            warn!("⚠️ Risk: Confidence {:.2} too low (< 0.6)", confidence);
            return false;
        }

        true
    }

    /// Check stop loss condition
    pub fn check_stop_loss(&self, position: &Position, current_price: f64) -> bool {
        // Calculate P&L %
        // For YES/NO tokens, price moves from entry_price.
        // If I bought YES at 0.6 and it goes to 0.5, I lost (0.5-0.6)/0.6 = -16.6%.
        // If I bought NO at 0.4 and it goes to 0.3 (meaning YES goes up), NO price is 0.3?
        // Wait, Polymarket 'no_price' is usually the price of the 'No' token.
        // So the logic is symmetric: (current - entry) / entry
        
        let pnl_pct = (current_price - position.entry_price) / position.entry_price;

        // Stop loss: e.g. -15% (represented as positive 0.15 in config, so check < -0.15)
        if pnl_pct < -self.config.stop_loss_pct {
            warn!(
                "🛑 Stop Loss Triggered! Market: {}, P/L: {:.2}%",
                position.market_id,
                pnl_pct * 100.0
            );
            return true;
        }

        false
    }

    pub fn add_position(
        &mut self,
        market_id: String,
        trade_id: String,
        side: String,
        size_usd: f64,
        entry_price: f64,
    ) {
        let position = Position {
            market_id: market_id.clone(),
            trade_id,
            side,
            size_usd,
            entry_price,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        self.positions.insert(market_id, position);
        info!("📝 Position added: size=${:.2}, price={:.4}", size_usd, entry_price);
    }

    pub fn remove_position(&mut self, market_id: &str) {
        if self.positions.remove(market_id).is_some() {
            info!("🗑️ Position removed for market {}", market_id);
        }
    }

    pub fn get_positions(&self) -> Vec<Position> {
        self.positions.values().cloned().collect()
    }

    /// Helper for legacy compatibility (if needed) but ideally unused now
    #[allow(dead_code)]
    pub fn validate_decision(
        &self,
        decision: &TradingDecision,
        market_id: &str,
    ) -> Option<TradingDecision> {
        // Use validate_entry logic but map it back to decision
        let capital = 1000.0;
        let size_usd = capital * decision.position_size_pct;
        
        if self.validate_entry(market_id, size_usd, decision.confidence) {
            Some(decision.clone())
        } else {
            None
        }
    }

    fn max_position_size(&self) -> f64 {
        // Assume $1000 capital for now (TODO: Fetch real balance)
        1000.0 * self.config.max_position_size_pct
    }
}
