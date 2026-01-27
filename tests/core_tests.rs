use polymarket_hft_agent::polymarket::OrderBook;
use polymarket_hft_agent::config::{RiskConfig, AgentConfig};
use polymarket_hft_agent::decision::RiskManager;

#[test]
fn test_orderbook_logic() {
    let mut book = OrderBook::new();
    
    // Test Imbalance
    book.update("BUY", 0.50, 100.0);
    book.update("SELL", 0.60, 100.0);
    assert_eq!(book.calculate_imbalance(), 0.0); // Balanced

    book.update("BUY", 0.50, 300.0); // Total Bid: 300, Total Ask: 100
    // (300 - 100) / (300 + 100) = 200 / 400 = 0.5
    assert_eq!(book.calculate_imbalance(), 0.5);

    // Test Best Quotes
    let (bid, ask) = book.best_quote();
    assert_eq!(bid, Some(0.50));
    assert_eq!(ask, Some(0.60));
}

#[test]
fn test_risk_manager() {
    let config = RiskConfig {
        max_position_size_pct: 10.0,
        max_portfolio_exposure_pct: 50.0,
        stop_loss_pct: 5.0,
    };
    
    let mut rm = RiskManager::new(config);
    
    // Test Entry Validation
    assert!(rm.validate_entry("mkt1", 5.0, 0.8)); // Valid size
    assert!(!rm.validate_entry("mkt1", 15.0, 0.8)); // Exceeds max pos size
    
    // Add position
    rm.add_position("mkt1".into(), "trade1".into(), "YES".into(), 100.0, 0.50);
    
    // Test Stop Loss
    // Entry 0.50. Stop loss 5% = 0.475.
    // Price drops to 0.47 -> Should trigger
    let position = rm.get_positions().pop().unwrap();
    assert!(rm.check_stop_loss(&position, 0.47));
    assert!(!rm.check_stop_loss(&position, 0.49));
}
