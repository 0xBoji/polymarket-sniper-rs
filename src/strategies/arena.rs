use bumpalo::Bump;
use crate::strategies::arbitrage::TradeAction;

/// Memory arena for zero-allocation hot path
/// Reuses memory across multiple decision cycles
pub struct StrategyArena {
    arena: Bump,
    decision_count: u64,
}

impl StrategyArena {
    pub fn new() -> Self {
        Self {
            arena: Bump::new(),
            decision_count: 0,
        }
    }
    
    /// Allocate a TradeAction in the arena
    /// This avoids heap allocation for each decision
    #[inline(always)]
    pub fn alloc_action(&self, action: TradeAction) -> &TradeAction {
        self.arena.alloc(action)
    }
    
    /// Reset the arena after processing a batch of decisions
    /// This bulk-frees all allocations at once
    pub fn reset(&mut self) {
        self.decision_count += 1;
        
        // Reset every 1000 decisions to avoid arena growing too large
        if self.decision_count % 1000 == 0 {
            self.arena.reset();
        }
    }
    
    /// Get current arena size in bytes
    pub fn allocated_bytes(&self) -> usize {
        self.arena.allocated_bytes()
    }
    
    /// Get number of decisions processed
    pub fn decision_count(&self) -> u64 {
        self.decision_count
    }
}

impl Default for StrategyArena {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_arena_allocation() {
        let arena = StrategyArena::new();
        
        let action = TradeAction::BuyBoth {
            market_id: "test".to_string(),
            yes_price: 0.4,
            no_price: 0.4,
            size_usd: 10.0,
            expected_profit_bps: 2000,
        };
        
        let allocated = arena.alloc_action(action);
        
        match allocated {
            TradeAction::BuyBoth { expected_profit_bps, .. } => {
                assert_eq!(*expected_profit_bps, 2000);
            }
            _ => panic!("Wrong action type"),
        }
    }
    
    #[test]
    fn test_arena_reset() {
        let mut arena = StrategyArena::new();
        
        // Allocate some actions
        for _ in 0..10 {
            let action = TradeAction::None;
            arena.alloc_action(action);
            arena.reset();
        }
        
        assert_eq!(arena.decision_count(), 10);
    }
}
