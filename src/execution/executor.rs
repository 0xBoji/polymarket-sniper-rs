use anyhow::Result;
use tracing::{info, warn};

use crate::strategies::risk::RiskManager;
use crate::strategies::types::TradingDecision;
use crate::polymarket::{MarketData, MarketInterface};
use crate::execution::flashbots::FlashbotsClient;

pub struct Executor {
    market_interface: Box<dyn MarketInterface>,
    flashbots_client: Option<FlashbotsClient>,
}

impl Executor {
    pub fn new(
        market_interface: Box<dyn MarketInterface>,
        flashbots_client: Option<FlashbotsClient>,
    ) -> Self {
        Self {
            market_interface,
            flashbots_client,
        }
    }

    /// Execute a trading decision
    pub async fn execute_trade(
        &self,
        decision: &TradingDecision,
        market: &MarketData,
        trade_id: &str,
        risk_manager: &mut RiskManager,
    ) -> Result<String> {
        if !decision.should_trade {
            info!("⏭️  Skipping trade for market: {}", market.question);
            return Ok("".to_string());
        }

        info!("🚀 Executing trade for market: {}", market.question);

        // Determine price (TODO: Use order book for better accuracy)
        let price = if decision.side == "YES" {
            market.yes_price
        } else {
            market.no_price
        };

        // Calculate actual position size in dollars
        let capital = self.market_interface.get_balance().await.unwrap_or(1000.0);
        let position_size_usd = capital * decision.position_size_pct;

        // Place order (regular submission - Flashbots is used for atomic bundles only)
        let order_id = self
            .market_interface
            .place_order(
                &market.id,
                &decision.side,
                price,
                position_size_usd,
            )
            .await?;

        info!(
            "✅ Order placed: {} {} @ ${:.4} (Size: ${:.2})",
            decision.side, market.question, price, position_size_usd
        );

        // Register position with risk manager
        risk_manager.add_position(
            trade_id.to_string(),
            market.id.clone(),
            decision.side.clone(),
            position_size_usd,
            price,
        );

        Ok(order_id)
    }

    /// Execute atomic arbitrage bundle (YES + NO together)
    /// This is the key method for MEV protection
    pub async fn execute_arbitrage_bundle(
        &self,
        market: &MarketData,
        yes_price: f64,
        no_price: f64,
        size_usd: f64,
        trade_id: &str,
        risk_manager: &mut RiskManager,
    ) -> Result<String> {
        // Check if Flashbots is enabled
        if let Some(_flashbots) = &self.flashbots_client {
            info!("⚡ Executing ATOMIC arbitrage bundle via Flashbots");
            
            // TODO: Build actual transactions for YES and NO orders
            // This requires:
            // 1. Creating TypedTransaction for each order
            // 2. Setting proper gas limits and prices
            // 3. Signing with wallet
            
            // For now, we'll use the regular interface and log that Flashbots would be used
            warn!("⚠️ Flashbots bundle creation not yet implemented - using regular execution");
            warn!("⚠️ TODO: Build TypedTransaction from Polymarket order data");
            
            // Fallback to regular execution
            self.execute_regular_arbitrage(market, yes_price, no_price, size_usd, trade_id, risk_manager).await
        } else {
            // No Flashbots - regular execution
            self.execute_regular_arbitrage(market, yes_price, no_price, size_usd, trade_id, risk_manager).await
        }
    }

    /// Regular (non-atomic) arbitrage execution
    async fn execute_regular_arbitrage(
        &self,
        market: &MarketData,
        yes_price: f64,
        no_price: f64,
        size_usd: f64,
        trade_id: &str,
        risk_manager: &mut RiskManager,
    ) -> Result<String> {
        info!("🔄 Executing regular arbitrage (non-atomic)");
        
        // Execute YES order
        let yes_order_id = self
            .market_interface
            .place_order(&market.id, "YES", yes_price, size_usd / 2.0)
            .await?;
        
        info!("✅ YES order placed: {}", yes_order_id);
        
        // Execute NO order
        let no_order_id = self
            .market_interface
            .place_order(&market.id, "NO", no_price, size_usd / 2.0)
            .await?;
        
        info!("✅ NO order placed: {}", no_order_id);
        
        // Register positions
        risk_manager.add_position(
            format!("{}_YES", trade_id),
            market.id.clone(),
            "YES".to_string(),
            size_usd / 2.0,
            yes_price,
        );
        
        risk_manager.add_position(
            format!("{}_NO", trade_id),
            market.id.clone(),
            "NO".to_string(),
            size_usd / 2.0,
            no_price,
        );
        
        Ok(format!("YES:{},NO:{}", yes_order_id, no_order_id))
    }

    /// Close a position
    pub async fn close_position(
        &self,
        market: &MarketData,
        side: &str,
        risk_manager: &mut RiskManager,
    ) -> Result<()> {
        info!("🔴 Closing position: {} {}", side, market.question);

        // Get current price
        let price = if side == "YES" {
            market.yes_price
        } else {
            market.no_price
        };

        // Find position size
        let positions = risk_manager.get_positions();
        let position = positions
            .iter()
            .find(|p| p.market_id == market.id && p.side == side)
            .ok_or_else(|| anyhow::anyhow!("Position not found"))?;

        // Place closing order (opposite side)
        let opposite_side = if side == "YES" { "NO" } else { "YES" };
        let _order_id = self
            .market_interface
            .place_order(&market.id, opposite_side, price, position.size_usd)
            .await?;

        // Remove position from risk manager
        risk_manager.remove_position(&market.id);

        info!("✅ Position closed successfully");
        Ok(())
    }
}
