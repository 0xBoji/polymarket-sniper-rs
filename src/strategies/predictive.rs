use crate::config::PredictiveConfig;
use crate::polymarket::MarketData;
use crate::pricefeed::BinanceClient;
use crate::strategies::arbitrage::TradeAction;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tracing::{debug, info};

pub struct PredictiveStrategy {
    config: PredictiveConfig,
    binance: Arc<BinanceClient>,
}

impl PredictiveStrategy {
    pub fn new(config: PredictiveConfig, binance: Arc<BinanceClient>) -> Self {
        Self { config, binance }
    }

    /// Check for opportunities using external price signals (Binance)
    /// Last-minute mode:
    /// - Crypto strike markets only
    /// - Time to expiry must be within configured final window
    /// - Binance must show directional edge beyond threshold
    pub async fn check_opportunity(&self, market: &MarketData) -> TradeAction {
        if !self.config.enabled {
            return TradeAction::None;
        }

        // 0. Last-minute filter
        let Some(end_date) = &market.end_date else {
            return TradeAction::None;
        };
        let Ok(end_dt) = DateTime::parse_from_rfc3339(end_date) else {
            return TradeAction::None;
        };
        let time_remaining = (end_dt.with_timezone(&Utc) - Utc::now()).num_seconds();
        if time_remaining <= 0 || time_remaining > self.config.final_window_sec as i64 {
            return TradeAction::None;
        }

        // 1. Identify if this is a supported Up/Down market
        let symbol = match BinanceClient::symbol_from_question(&market.question) {
            Some(s) => s,
            None => return TradeAction::None,
        };

        // 2. Extract strike price from question
        // Example: "Bitcoin above $65,500.00 at 5:00 PM ET?"
        let strike_price = match self.extract_strike_price(&market.question) {
            Some(p) => p,
            None => {
                debug!("Failed to extract strike price from: {}", market.question);
                return TradeAction::None;
            }
        };

        // 3. Get current Binance price
        let binance_price = match self.binance.get_price(symbol).await {
            Ok(p) => p,
            Err(e) => {
                debug!("Failed to fetch Binance price for {}: {}", symbol, e);
                return TradeAction::None;
            }
        };

        // 4. Compare and determine signal
        let q = market.question.to_lowercase();
        let is_above_bet = q.contains("above") || q.contains("over") || q.contains("greater than");
        let is_below_bet = q.contains("below") || q.contains("under") || q.contains("less than");
        if !is_above_bet && !is_below_bet {
            return TradeAction::None;
        }

        let price_diff_pct = ((binance_price - strike_price) / strike_price).abs() * 100.0;
        let threshold = self.config.binance_signal_threshold_pct;
        let binance_edge_up = binance_price > strike_price * (1.0 + threshold / 100.0);
        let binance_edge_down = binance_price < strike_price * (1.0 - threshold / 100.0);

        let (side, entry_price) = if is_above_bet {
            if binance_edge_up {
                ("YES", market.yes_price)
            } else if binance_edge_down {
                ("NO", market.no_price)
            } else {
                return TradeAction::None;
            }
        } else if binance_edge_down {
            ("YES", market.yes_price)
        } else if binance_edge_up {
            ("NO", market.no_price)
        } else {
            return TradeAction::None;
        };

        if entry_price <= 0.0 || entry_price > self.config.max_entry_price {
            debug!(
                "Skip predictive entry ({}): entry price {:.4} outside allowed range (<= {:.4})",
                market.question, entry_price, self.config.max_entry_price
            );
            return TradeAction::None;
        }

        info!(
            "🚀 LAST-MINUTE BINANCE SIGNAL ({}): {} | Binance {:.2} vs Strike {:.2} | Diff {:.2}% | T-{}s",
            side,
            market.question,
            binance_price,
            strike_price,
            price_diff_pct,
            time_remaining
        );
        TradeAction::Snipe {
            market_id: market.id.clone(),
            side: side.to_string(),
            price: entry_price,
            size_usd: 1.0,
        }
    }

    fn extract_strike_price(&self, question: &str) -> Option<f64> {
        // Regex-free simple extraction
        // Look for '$' and then the number
        let parts: Vec<&str> = question.split('$').collect();
        if parts.len() < 2 {
            // Some questions might not use '$'
            // Example: "Bitcoin greater than 65500.00?"
            // We'll try to find the first numeric part
            for word in question.split_whitespace() {
                let cleaned = word.trim_matches(|c: char| !c.is_digit(10) && c != '.' && c != ',');
                if let Ok(p) = cleaned.replace(',', "").parse::<f64>() {
                    // Sanity check: price must be positive
                    if p > 0.0 {
                        return Some(p);
                    }
                }
            }
            return None;
        }

        let price_part = parts[1].split_whitespace().next()?;
        let cleaned = price_part.trim_matches(|c: char| !c.is_digit(10) && c != '.' && c != ',');
        cleaned.replace(',', "").parse::<f64>().ok()
    }
}
