use crate::config::ExpirationConfig;
use crate::polymarket::MarketData;
use crate::strategies::arbitrage::TradeAction;
use chrono::{DateTime, Utc};
use tracing::{debug, info};

pub struct ExpirationStrategy {
    config: ExpirationConfig,
}

impl ExpirationStrategy {
    pub fn new(config: ExpirationConfig) -> Self {
        Self { config }
    }

    /// Check for opportunities in markets near expiration
    /// Logic:
    /// 1. Time remaining < max_time_remaining (e.g. 60s)
    /// 2. Winning probability > min_win_prob (e.g. 0.90) based on price
    /// 3. Price < target_price (e.g. 0.99)
    pub fn check_opportunity(&self, market: &MarketData) -> TradeAction {
        if !self.config.enabled {
            return TradeAction::None;
        }

        // 1. Check Time Remaining
        let end_date_str = match &market.end_date {
            Some(d) => d,
            None => return TradeAction::None,
        };

        let time_remaining = match DateTime::parse_from_rfc3339(end_date_str) {
            Ok(dt) => {
                let now = Utc::now();
                let end = dt.with_timezone(&Utc);
                (end - now).num_seconds()
            }
            Err(_) => return TradeAction::None, // Invalid date format
        };

        if time_remaining <= 0 || time_remaining > self.config.max_time_remaining_sec as i64 {
            // debug!("⏳ Market {} time remaining: {}s (Target: < {}s)", market.id, time_remaining, self.config.max_time_remaining_sec);
            return TradeAction::None;
        }

        debug!(
            "⚡ Expiration Candidate: {} ({}s remaining)",
            market.question, time_remaining
        );

        // 2. Identify Winning Side & Check Rules
        // Rule: Price must be > min_price (highly likely to win) AND < target_price (profitable)

        let yes_price = market.yes_price;
        let no_price = market.no_price;

        // Check YES
        if yes_price >= self.config.min_price && yes_price < self.config.target_price {
            let profit_bps = ((1.0 - yes_price) * 10000.0) as i32;
            info!(
                "🎯 EXPIRATION SIGNAL (YES): {} | Price: {:.4} | Profit: {} bps | Time: {}s",
                market.question, yes_price, profit_bps, time_remaining
            );

            return TradeAction::Snipe {
                market_id: market.id.clone(),
                side: "YES".to_string(),
                price: yes_price,
                size_usd: 1.0, // Default size, will be capped by balance
            };
        }

        // Check NO
        if no_price >= self.config.min_price && no_price < self.config.target_price {
            let profit_bps = ((1.0 - no_price) * 10000.0) as i32;
            info!(
                "🎯 EXPIRATION SIGNAL (NO): {} | Price: {:.4} | Profit: {} bps | Time: {}s",
                market.question, no_price, profit_bps, time_remaining
            );

            return TradeAction::Snipe {
                market_id: market.id.clone(),
                side: "NO".to_string(),
                price: no_price,
                size_usd: 1.0,
            };
        }

        TradeAction::None
    }
}
