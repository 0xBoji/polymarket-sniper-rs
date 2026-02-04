use crate::polymarket::{MarketData, OrderBook, OrderLevel};
use crate::config::ArbitrageConfig;
use tracing::{debug, info};
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
    Snipe {
        market_id: String,
        side: String, // "YES" or "NO"
        price: f64,
        size_usd: f64,
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
    /// Optimized with branchless code and early returns
    #[inline(always)]
    pub fn check_opportunity(&self, market: &MarketData) -> TradeAction {
        let yes_ask = market.yes_price;
        let no_ask = market.no_price;

        // Branchless validation: both prices must be positive
        // If either is <= 0, total_cost will be invalid
        let total_cost = yes_ask + no_ask;
        
        // Polymarket fees: ~0.2% maker + ~0.2% taker = 0.4% per trade
        // For arbitrage (buy YES + buy NO), we pay fees twice = 0.8% total
        const FEE_PER_TRADE_BPS: i32 = 40;  // 0.4% = 40 bps
        const TOTAL_FEE_BPS: i32 = FEE_PER_TRADE_BPS * 2;  // 80 bps for both trades
        
        // Calculate spread AFTER fees
        let spread = 1.0 - total_cost;
        let spread_bps = (spread * 10000.0) as i32;
        let net_spread_bps = spread_bps - TOTAL_FEE_BPS;

        // DEBUG: Sample 0.1% of checks to ensure we are seeing correct prices
        if rand::random::<f64>() < 0.001 {
             info!("🔍 SAMPLE CHECK [{}]: Yes={:.3} No={:.3} Cost={:.3} Spread={}bps Fees={}bps Net={}bps", 
                market.question, yes_ask, no_ask, total_cost, spread_bps, TOTAL_FEE_BPS, net_spread_bps);
        }

        // Early return if no opportunity after fees (most common case)
        if net_spread_bps <= self.config.min_edge_bps {
            // Log "Close calls" (e.g. within 50bps of target) to show it's working
            if net_spread_bps > (self.config.min_edge_bps - 50) {
                 debug!("👀 CLOSE CALL [{}]: Net Edge {} bps (Target {})", 
                    market.question, net_spread_bps, self.config.min_edge_bps);
            }
            return TradeAction::None;
        }

        if yes_ask <= 0.0 || no_ask <= 0.0 {
            debug!("⚠️ Missing prices for {}: YES={:.4}, NO={:.4}", market.id, yes_ask, no_ask);
            return TradeAction::None;
        }

        // Hot path: calculate position size using NET spread (after fees)
        let size_usd = self.calculate_position_size(
            net_spread_bps,
            &market.id,
            0,
            true,
        );
        
        TradeAction::BuyBoth {
            market_id: market.id.clone(),
            yes_price: yes_ask,
            no_price: no_ask,
            size_usd,
            expected_profit_bps: net_spread_bps,  // Net profit after fees
        }
    }
    
    /// Calculate optimal position size based on edge and risk parameters
    /// Optimized with const capital and inline hint
    #[inline(always)]
    fn calculate_position_size(
        &self,
        edge_bps: i32,
        market_id: &str,
        slippage_bps: i32,
        is_atomic: bool,
    ) -> f64 {
        // Fast path: fixed sizing (no allocation)
        let sizer = match &self.position_sizer {
            Some(s) => s,
            None => return self.config.max_position_size_usd,
        };
        
        // Dynamic sizing using Kelly Criterion
        const CAPITAL: f64 = 1000.0; // Const for compiler optimization
        let win_prob = estimate_win_probability(is_atomic, slippage_bps);
        let volatility = estimate_volatility(market_id);
        
        sizer.calculate_optimal_size(edge_bps, win_prob, CAPITAL, volatility)
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
