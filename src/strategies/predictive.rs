use crate::polymarket::MarketData;
use crate::config::PredictiveConfig;
use crate::strategies::arbitrage::TradeAction;
use crate::pricefeed::BinanceClient;
use tracing::{debug, info};
use std::sync::Arc;

pub struct PredictiveStrategy {
    config: PredictiveConfig,
    binance: Arc<BinanceClient>,
}

impl PredictiveStrategy {
    pub fn new(config: PredictiveConfig, binance: Arc<BinanceClient>) -> Self {
        Self { config, binance }
    }

    /// Check for opportunities using external price signals (Binance)
    pub async fn check_opportunity(&self, market: &MarketData) -> TradeAction {
        if !self.config.enabled {
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
        // We look for "Above" or "Over" in the question
        let is_above_bet = market.question.to_lowercase().contains("above") || 
                          market.question.to_lowercase().contains("over") ||
                          market.question.to_lowercase().contains("greater than");

        let price_diff_pct = ((binance_price - strike_price) / strike_price).abs() * 100.0;

        // If Binance price is significantly above/below strike, we have a signal
        if is_above_bet {
            if binance_price > strike_price * (1.0 + self.config.binance_signal_threshold_pct / 100.0) {
                // Predictive YES
                if market.yes_price < 0.99 {
                    info!("🚀 PREDICTIVE SIGNAL (YES): {} | Binance: {:.2} | Strike: {:.2} | Diff: {:.2}%", 
                        market.question, binance_price, strike_price, price_diff_pct);
                    return TradeAction::Snipe {
                        market_id: market.id.clone(),
                        side: "YES".to_string(),
                        price: market.yes_price,
                        size_usd: 1.0, // Default size
                    };
                }
            } else if binance_price < strike_price * (1.0 - self.config.binance_signal_threshold_pct / 100.0) {
                // Predictive NO
                if market.no_price < 0.99 {
                    info!("🚀 PREDICTIVE SIGNAL (NO): {} | Binance: {:.2} | Strike: {:.2} | Diff: {:.2}%", 
                        market.question, binance_price, strike_price, price_diff_pct);
                    return TradeAction::Snipe {
                        market_id: market.id.clone(),
                        side: "NO".to_string(),
                        price: market.no_price,
                        size_usd: 1.0,
                    };
                }
            }
        }

        TradeAction::None
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
