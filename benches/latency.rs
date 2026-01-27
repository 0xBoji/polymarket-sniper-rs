use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use polymarket_hft_agent::polymarket::{MarketData, OrderBook, OrderLevel};
use polymarket_hft_agent::strategies::arbitrage::ArbitrageStrategy;
use polymarket_hft_agent::strategies::position_sizing::PositionSizer;
use polymarket_hft_agent::config::ArbitrageConfig;

fn create_mock_orderbook(depth: usize) -> OrderBook {
    let mut orderbook = OrderBook::new();
    
    // Create bids (descending prices)
    for i in 0..depth {
        orderbook.bids.push(OrderLevel {
            price: 0.50 - (i as f64 * 0.01),
            size: 100.0 + (i as f64 * 10.0),
        });
    }
    
    // Create asks (ascending prices)
    for i in 0..depth {
        orderbook.asks.push(OrderLevel {
            price: 0.51 + (i as f64 * 0.01),
            size: 100.0 + (i as f64 * 10.0),
        });
    }
    
    orderbook
}

fn benchmark_arbitrage_strategy(c: &mut Criterion) {
    let config = ArbitrageConfig {
        min_edge_bps: 200,
        max_position_size_usd: 10.0,
        use_dynamic_sizing: false,
        kelly_fraction: 0.25,
        min_position_pct: 0.01,
        max_position_pct: 0.10,
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

fn benchmark_orderbook_analysis(c: &mut Criterion) {
    let config = ArbitrageConfig {
        min_edge_bps: 200,
        max_position_size_usd: 10.0,
        use_dynamic_sizing: false,
        kelly_fraction: 0.25,
        min_position_pct: 0.01,
        max_position_pct: 0.10,
    };
    let strategy = ArbitrageStrategy::new(config);

    let mut group = c.benchmark_group("orderbook_analysis");

    // Test with different orderbook depths
    for depth in [5, 10, 20, 50].iter() {
        let orderbook = create_mock_orderbook(*depth);
        
        group.bench_with_input(
            BenchmarkId::new("analyze_depth", depth),
            &orderbook,
            |b, ob| {
                b.iter(|| {
                    black_box(strategy.analyze_orderbook_depth(black_box(ob), 100.0));
                })
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("calculate_slippage", depth),
            &orderbook,
            |b, ob| {
                b.iter(|| {
                    black_box(strategy.calculate_slippage(black_box(ob), 100.0));
                })
            },
        );
    }

    group.finish();
}

fn benchmark_position_sizing(c: &mut Criterion) {
    let sizer = PositionSizer::new(0.25, 0.01, 0.10);
    
    let mut group = c.benchmark_group("position_sizing");

    // Test Kelly calculation with different edges
    for edge_bps in [100, 200, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("kelly_calculation", edge_bps),
            edge_bps,
            |b, &edge| {
                b.iter(|| {
                    black_box(sizer.calculate_optimal_size(
                        black_box(edge),
                        black_box(0.95),
                        black_box(1000.0),
                        black_box(0.10),
                    ));
                })
            },
        );
    }

    group.finish();
}

fn benchmark_full_pipeline(c: &mut Criterion) {
    // Test with dynamic sizing enabled
    let config_dynamic = ArbitrageConfig {
        min_edge_bps: 200,
        max_position_size_usd: 10.0,
        use_dynamic_sizing: true,
        kelly_fraction: 0.25,
        min_position_pct: 0.01,
        max_position_pct: 0.10,
    };
    let strategy_dynamic = ArbitrageStrategy::new(config_dynamic);

    // Test with dynamic sizing disabled
    let config_fixed = ArbitrageConfig {
        min_edge_bps: 200,
        max_position_size_usd: 10.0,
        use_dynamic_sizing: false,
        kelly_fraction: 0.25,
        min_position_pct: 0.01,
        max_position_pct: 0.10,
    };
    let strategy_fixed = ArbitrageStrategy::new(config_fixed);

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

    let mut group = c.benchmark_group("full_pipeline");

    group.bench_function("fixed_sizing", |b| {
        b.iter(|| {
            black_box(strategy_fixed.check_opportunity(black_box(&market_arb)));
        })
    });

    group.bench_function("dynamic_sizing", |b| {
        b.iter(|| {
            black_box(strategy_dynamic.check_opportunity(black_box(&market_arb)));
        })
    });

    group.finish();
}

fn benchmark_orderbook_helpers(c: &mut Criterion) {
    let orderbook = create_mock_orderbook(20);
    
    let mut group = c.benchmark_group("orderbook_helpers");

    group.bench_function("best_bid", |b| {
        b.iter(|| {
            black_box(orderbook.best_bid());
        })
    });

    group.bench_function("best_ask", |b| {
        b.iter(|| {
            black_box(orderbook.best_ask());
        })
    });

    group.bench_function("total_bid_liquidity", |b| {
        b.iter(|| {
            black_box(orderbook.total_bid_liquidity());
        })
    });

    group.bench_function("total_ask_liquidity", |b| {
        b.iter(|| {
            black_box(orderbook.total_ask_liquidity());
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_arbitrage_strategy,
    benchmark_orderbook_analysis,
    benchmark_position_sizing,
    benchmark_full_pipeline,
    benchmark_orderbook_helpers
);
criterion_main!(benches);
