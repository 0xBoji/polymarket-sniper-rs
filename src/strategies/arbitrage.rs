use crate::polymarket::{MarketData, OrderBook, OrderLevel};
use crate::config::ArbitrageConfig;
use crate::strategies::position_sizing::{PositionSizer, estimate_win_probability, estimate_volatility};

#[derive(Debug)]
pub enum TradeAction {
    BuyBoth {
        market_id: String,
        yes_price: f64,
        no_price: f64,
        size_usd: f64,
        expected_profit_bps: i32,
    },
    None,
}

/// Orderbook depth analysis result
#[derive(Debug, Clone)]
pub struct OrderbookDepth {
    pub total_bid_liquidity: f64,
    pub total_ask_liquidity: f64,
    pub weighted_bid_price: f64,
    pub weighted_ask_price: f64,
    pub slippage_bps: i32,
    pub imbalance_ratio: f64, // bid_depth / ask_depth
}

pub struct ArbitrageStrategy {
    config: ArbitrageConfig,
    position_sizer: Option<PositionSizer>,
}

impl ArbitrageStrategy {
    pub fn new(config: ArbitrageConfig) -> Self {
        // Initialize position sizer if dynamic sizing is enabled
        let position_sizer = if config.use_dynamic_sizing {
            Some(PositionSizer::new(
                config.kelly_fraction,
                config.min_position_pct,
                config.max_position_pct,
            ))
        } else {
            None
        };
        
        Self { 
            config,
            position_sizer,
        }
    }

    /// Check for arbitrage opportunity using simple best bid/ask
    /// Now with dynamic position sizing based on Kelly Criterion
    pub fn check_opportunity(&self, market: &MarketData) -> TradeAction {
        // Basic Intra-Market Arbitrage:
        // If Price(YES) + Price(NO) < 1.0, there is a guaranteed profit (ignoring fees for a moment).
        // Realistically: Price(YES) + Price(NO) < 1.0 - fees - target_margin
        
        let yes_ask = market.yes_price; // Assuming yes_price is best ask for now
        let no_ask = market.no_price;   // Assuming no_price is best ask for now

        if yes_ask <= 0.0 || no_ask <= 0.0 {
            return TradeAction::None;
        }

        let total_cost = yes_ask + no_ask;
        let spread = 1.0 - total_cost;
        let spread_bps = (spread * 10000.0) as i32;

        if spread_bps > self.config.min_edge_bps {
            // Calculate position size
            let size_usd = self.calculate_position_size(
                spread_bps,
                &market.id,
                0, // No slippage for simple best bid/ask
                true, // Assume atomic execution
            );
            
            return TradeAction::BuyBoth {
                market_id: market.id.clone(),
                yes_price: yes_ask,
                no_price: no_ask,
                size_usd,
                expected_profit_bps: spread_bps,
            };
            
        }

        TradeAction::None
    }
    
    /// Calculate optimal position size based on edge and risk parameters
    fn calculate_position_size(
        &self,
        edge_bps: i32,
        market_id: &str,
        slippage_bps: i32,
        is_atomic: bool,
    ) -> f64 {
        if let Some(sizer) = &self.position_sizer {
            // Dynamic sizing using Kelly Criterion
            let capital = 1000.0; // TODO: Get from account balance
            let win_prob = estimate_win_probability(is_atomic, slippage_bps);
            let volatility = estimate_volatility(market_id);
            
            sizer.calculate_optimal_size(edge_bps, win_prob, capital, volatility)
        } else {
            // Fixed sizing (fallback)
            self.config.max_position_size_usd
        }
    }

    /// Analyze full orderbook depth for a given order size
    pub fn analyze_orderbook_depth(&self, orderbook: &OrderBook, order_size_usd: f64) -> OrderbookDepth {
        // Calculate weighted average prices based on order size
        let (weighted_bid, _bid_liquidity) = self.calculate_weighted_price(orderbook.bid_levels(), order_size_usd);
        let (weighted_ask, _ask_liquidity) = self.calculate_weighted_price(orderbook.ask_levels(), order_size_usd);
        
        // Calculate slippage (difference between best price and weighted average)
        let best_bid = orderbook.best_bid().unwrap_or(0.0);
        let best_ask = orderbook.best_ask().unwrap_or(1.0);
        
        let bid_slippage = if best_bid > 0.0 {
            ((best_bid - weighted_bid) / best_bid * 10000.0) as i32
        } else {
            0
        };
        
        let ask_slippage = if best_ask > 0.0 {
            ((weighted_ask - best_ask) / best_ask * 10000.0) as i32
        } else {
            0
        };
        
        let total_slippage_bps = bid_slippage + ask_slippage;
        
        // Calculate liquidity imbalance
        let total_bid_liq = orderbook.total_bid_liquidity();
        let total_ask_liq = orderbook.total_ask_liquidity();
        let imbalance_ratio = if total_ask_liq > 0.0 {
            total_bid_liq / total_ask_liq
        } else {
            0.0
        };
        
        OrderbookDepth {
            total_bid_liquidity: total_bid_liq,
            total_ask_liquidity: total_ask_liq,
            weighted_bid_price: weighted_bid,
            weighted_ask_price: weighted_ask,
            slippage_bps: total_slippage_bps,
            imbalance_ratio,
        }
    }
    
    /// Calculate weighted average price for a given order size
    /// Returns (weighted_price, total_liquidity_consumed)
    fn calculate_weighted_price(&self, levels: &[OrderLevel], order_size_usd: f64) -> (f64, f64) {
        if levels.is_empty() {
            return (0.0, 0.0);
        }
        
        let mut remaining_size = order_size_usd;
        let mut total_cost = 0.0;
        let mut total_size_filled = 0.0;
        
        for level in levels {
            if remaining_size <= 0.0 {
                break;
            }
            
            let size_at_level = level.size.min(remaining_size);
            total_cost += size_at_level * level.price;
            total_size_filled += size_at_level;
            remaining_size -= size_at_level;
        }
        
        if total_size_filled > 0.0 {
            (total_cost / total_size_filled, total_size_filled)
        } else {
            (levels[0].price, 0.0)
        }
    }
    
    /// Check for arbitrage opportunity using full orderbook depth
    pub fn check_orderbook_opportunity(
        &self,
        market_id: &str,
        yes_orderbook: &OrderBook,
        no_orderbook: &OrderBook,
        order_size_usd: f64,
    ) -> TradeAction {
        // Analyze depth for both YES and NO orderbooks
        let yes_depth = self.analyze_orderbook_depth(yes_orderbook, order_size_usd / 2.0);
        let no_depth = self.analyze_orderbook_depth(no_orderbook, order_size_usd / 2.0);
        
        // Use weighted ask prices (we're buying)
        let yes_ask = yes_depth.weighted_ask_price;
        let no_ask = no_depth.weighted_ask_price;
        
        if yes_ask <= 0.0 || no_ask <= 0.0 {
            return TradeAction::None;
        }
        
        // Calculate total cost including slippage
        let total_cost = yes_ask + no_ask;
        let spread = 1.0 - total_cost;
        let spread_bps = (spread * 10000.0) as i32;
        
        // Adjust for slippage
        let total_slippage = yes_depth.slippage_bps + no_depth.slippage_bps;
        let net_edge_bps = spread_bps - total_slippage;
        
        if net_edge_bps > self.config.min_edge_bps {
            return TradeAction::BuyBoth {
                market_id: market_id.to_string(),
                yes_price: yes_ask,
                no_price: no_ask,
                size_usd: order_size_usd,
                expected_profit_bps: net_edge_bps,
            };
        }
        
        TradeAction::None
    }
    
    /// Calculate slippage for a given order size
    pub fn calculate_slippage(&self, orderbook: &OrderBook, order_size_usd: f64) -> i32 {
        let depth = self.analyze_orderbook_depth(orderbook, order_size_usd);
        depth.slippage_bps
    }
}
