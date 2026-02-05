use polymarket_hft_agent::pricefeed::BinanceClient;

fn main() {
    let test_cases = vec![
        ("Will Bitcoin hit $100k?", Some("BTCUSDT")),
        ("Will Ethereum flip Bitcoin?", Some("ETHUSDT")), // Should match ETH first or be handled logically
        ("Will Dogecoin reach $1?", Some("DOGEUSDT")),
        ("Will BNB remain above $600?", Some("BNBUSDT")),
        ("Can Cardano hit $5?", Some("ADAUSDT")),
        ("Will Solana ETF be approved?", Some("SOLUSDT")),
        ("XRP vs SEC case resolution?", Some("XRPUSDT")),
        ("Will Avalanche survive?", Some("AVAXUSDT")),
        ("Matic to $2?", Some("MATICUSDT")),
        ("Will Chainlink pump?", Some("LINKUSDT")),
        ("Random question about politics", None),
    ];

    println!("🧪 Testing Predictive Mapping Logic...\n");

    let mut passed = 0;
    let mut failed = 0;

    for (question, expected) in test_cases {
        let result = BinanceClient::symbol_from_question(question);
        
        let status = if result == expected {
            passed += 1;
            "✅ PASS"
        } else {
            failed += 1;
            "❌ FAIL"
        };

        println!("{} '{}' -> {:?} (Expected: {:?})", status, question, result, expected);
    }

    println!("\n📊 Result: {} Passed, {} Failed", passed, failed);
}
