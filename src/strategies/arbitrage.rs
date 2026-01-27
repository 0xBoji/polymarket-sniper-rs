use crate::polymarket::MarketData;
use crate::config::ArbitrageConfig;
// use crate::strategies::types::TradingDecision; // Unused

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

pub struct ArbitrageStrategy {
    config: ArbitrageConfig,
}

impl ArbitrageStrategy {
    pub fn new(config: ArbitrageConfig) -> Self {
        Self { config }
    }

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
            // Found opportunity!
            // E.g. YES=0.4, NO=0.5 -> Cost=0.9 -> Profit=0.1 (1000 bps)
            
            return TradeAction::BuyBoth {
                market_id: market.id.clone(),
                yes_price: yes_ask,
                no_price: no_ask,
                size_usd: self.config.max_position_size_usd,
                expected_profit_bps: spread_bps,
            };
            
        }

        TradeAction::None
    }


    pub fn check_orderbook_opportunity(&self, _market_id: &str, _best_bid: f64, best_ask: f64) -> TradeAction {
        // NOTE: OrderbookUpdate gives us ONE side (asset_id)
        // We need to know if this asset_id is YES or NO for the market.
        // This requires state context that checks which asset_id corresponds to YES/NO.
        // For now, let's assume the Agent passes us the BEST Prices for YES and NO it has tracked.
        
        let _yes_price = best_ask; // Placeholder logic
        // Actually, this logic belongs in Agent which aggregates the L2 updates.
        // The Strategy should just take (YES_PRICE, NO_PRICE) and return decision.
        
        TradeAction::None
    }
}
