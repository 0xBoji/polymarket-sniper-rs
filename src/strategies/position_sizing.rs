/// Dynamic position sizing using Kelly Criterion and risk adjustments

pub struct PositionSizer {
    kelly_fraction: f64,    // Fractional Kelly (e.g., 0.25 for quarter-Kelly)
    max_position_pct: f64,  // Maximum position as % of capital
    min_position_pct: f64,  // Minimum position as % of capital
}

impl PositionSizer {
    pub fn new(kelly_fraction: f64, min_position_pct: f64, max_position_pct: f64) -> Self {
        Self {
            kelly_fraction,
            min_position_pct,
            max_position_pct,
        }
    }
    
    /// Calculate optimal position size using Kelly Criterion
    /// 
    /// Kelly Formula: f = (p * b - q) / b
    /// where:
    /// - f = fraction of capital to bet
    /// - p = probability of winning
    /// - q = probability of losing (1 - p)
    /// - b = odds (profit / loss ratio)
    /// 
    /// For arbitrage:
    /// - p ≈ 1.0 (near certain if executed atomically)
    /// - b = edge_bps / (10000 - edge_bps)
    #[inline(always)]
    pub fn calculate_optimal_size(
        &self,
        edge_bps: i32,
        win_probability: f64,
        capital: f64,
        volatility: f64,
    ) -> f64 {
        // Early return for invalid inputs (most common in testing)
        if edge_bps <= 0 || win_probability <= 0.0 || capital <= 0.0 {
            return 0.0;
        }
        
        // Convert edge from bps to decimal
        let edge = edge_bps as f64 / 10000.0;
        
        // Calculate odds (profit/loss ratio)
        // For arbitrage: if edge is 100bps (1%), odds = 0.01 / 0.99 ≈ 0.0101
        let odds = edge / (1.0 - edge);
        
        // Kelly formula
        let p = win_probability;
        let q = 1.0 - p;
        let kelly_fraction_raw = (p * odds - q) / odds;
        
        // Apply fractional Kelly for safety
        let kelly_fraction_adjusted = kelly_fraction_raw * self.kelly_fraction;
        
        // Adjust for volatility (reduce size in high volatility)
        let volatility_adjusted = self.adjust_for_volatility(kelly_fraction_adjusted, volatility);
        
        // Convert to USD
        let size_usd = capital * volatility_adjusted;
        
        // Apply risk limits
        self.apply_risk_limits(size_usd, capital)
    }
    
    /// Adjust position size based on market volatility
    #[inline(always)]
    fn adjust_for_volatility(&self, kelly_fraction: f64, volatility: f64) -> f64 {
        // Volatility adjustment factor
        // Higher volatility -> reduce position size
        // volatility is expected to be a value like 0.1 (10% volatility)
        
        if volatility <= 0.0 {
            return kelly_fraction;
        }
        
        // Simple volatility scaling: reduce by volatility percentage
        // If volatility is 20% (0.2), reduce kelly by 20%
        let volatility_factor = 1.0 - (volatility * 0.5).min(0.5); // Cap at 50% reduction
        
        kelly_fraction * volatility_factor
    }
    
    /// Apply min/max position size constraints
    #[inline(always)]
    fn apply_risk_limits(&self, size_usd: f64, capital: f64) -> f64 {
        let min_size = capital * self.min_position_pct;
        let max_size = capital * self.max_position_pct;
        
        size_usd.max(min_size).min(max_size)
    }
    
    /// Calculate Sharpe-optimal position size (alternative to Kelly)
    /// Uses mean-variance optimization
    #[allow(dead_code)]
    pub fn calculate_sharpe_optimal_size(
        &self,
        expected_return: f64,
        volatility: f64,
        capital: f64,
        risk_free_rate: f64,
    ) -> f64 {
        if volatility <= 0.0 {
            return 0.0;
        }
        
        // Sharpe ratio
        let sharpe = (expected_return - risk_free_rate) / volatility;
        
        // Optimal leverage = Sharpe / volatility
        let optimal_leverage = sharpe / volatility;
        
        // Convert to position size
        let size_fraction = optimal_leverage * self.kelly_fraction;
        let size_usd = capital * size_fraction;
        
        self.apply_risk_limits(size_usd, capital)
    }
    
    /// Simple fixed-fraction sizing (fallback)
    pub fn calculate_fixed_fraction(&self, capital: f64, fraction: f64) -> f64 {
        let size_usd = capital * fraction;
        self.apply_risk_limits(size_usd, capital)
    }
}

/// Helper function to estimate win probability for arbitrage
/// For atomic arbitrage, this should be very high (0.95-0.99)
/// For non-atomic, adjust based on execution risk
#[inline(always)]
pub fn estimate_win_probability(is_atomic: bool, slippage_bps: i32) -> f64 {
    if is_atomic {
        // Atomic execution via Flashbots - very high probability
        0.98
    } else {
        // Non-atomic - adjust for slippage and execution risk
        let base_prob = 0.90;
        let slippage_penalty = (slippage_bps as f64 / 10000.0) * 0.5;
        (base_prob - slippage_penalty).max(0.5)
    }
}

/// Estimate market volatility from recent price movements
/// This is a placeholder - in production, calculate from historical data
#[inline(always)]
pub fn estimate_volatility(_market_id: &str) -> f64 {
    // Default to 10% volatility
    // TODO: Calculate from historical price data
    0.10
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_kelly_calculation() {
        let sizer = PositionSizer::new(0.25, 0.01, 0.10);
        
        // Test with 200 bps edge, 95% win prob, $1000 capital, 10% volatility
        let size = sizer.calculate_optimal_size(200, 0.95, 1000.0, 0.10);
        
        assert!(size > 0.0);
        assert!(size <= 100.0); // Should not exceed 10% of capital
        
        info!("Kelly size for 200bps edge: ${:.2}", size);
    }
    
    #[test]
    fn test_risk_limits() {
        let sizer = PositionSizer::new(0.25, 0.01, 0.10);
        
        // Test min limit
        let size_small = sizer.apply_risk_limits(5.0, 1000.0);
        assert_eq!(size_small, 10.0); // Should be clamped to 1% min
        
        // Test max limit
        let size_large = sizer.apply_risk_limits(200.0, 1000.0);
        assert_eq!(size_large, 100.0); // Should be clamped to 10% max
    }
}
