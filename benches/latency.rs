use criterion::{black_box, criterion_group, criterion_main, Criterion};
use polymarket_hft_agent::polymarket::MarketData;
use polymarket_hft_agent::strategies::arbitrage::ArbitrageStrategy;
use polymarket_hft_agent::config::ArbitrageConfig;

fn benchmark_strategy(c: &mut Criterion) {
    let config = ArbitrageConfig {
        min_edge_bps: 200,
        max_position_size_usd: 10.0,
    };
    let strategy = ArbitrageStrategy::new(config);

    // Case 1: No Opportunity
    let market_no_arb = MarketData {
        id: "market_no".to_string(),
        question: "No Arb Market".to_string(),
        end_date: None,
        volume: 10000.0,
        liquidity: 5000.0,
        yes_price: 0.55,
        no_price: 0.55,
        description: None,
        order_book_imbalance: 0.0,
        best_bid: 0.0,
        best_ask: 0.0,
        asset_ids: vec![],
    };

    // Case 2: Profitable Opportunity (0.4 + 0.4 = 0.8 < 1.0)
    let market_arb = MarketData {
        id: "market_arb".to_string(),
        question: "Arb Market".to_string(),
        end_date: None,
        volume: 10000.0,
        liquidity: 5000.0,
        yes_price: 0.40,
        no_price: 0.40,
        description: None,
        order_book_imbalance: 0.0,
        best_bid: 0.0,
        best_ask: 0.0,
        asset_ids: vec![],
    };

    let mut group = c.benchmark_group("arbitrage_strategy");

    group.bench_function("check_no_opportunity", |b| {
        b.iter(|| {
            black_box(strategy.check_opportunity(black_box(&market_no_arb)));
        })
    });

    group.bench_function("check_profitable_opportunity", |b| {
        b.iter(|| {
            black_box(strategy.check_opportunity(black_box(&market_arb)));
        })
    });

    group.finish();
}

criterion_group!(benches, benchmark_strategy);
criterion_main!(benches);
