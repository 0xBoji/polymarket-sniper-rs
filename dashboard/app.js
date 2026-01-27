// Dashboard JavaScript
let pnlChart = null;
const pnlHistory = [];

// Initialize Chart
function initChart() {
    const ctx = document.getElementById('pnl-chart').getContext('2d');
    pnlChart = new Chart(ctx, {
        type: 'line',
        data: {
            labels: [],
            datasets: [{
                label: 'Total PnL ($)',
                data: [],
                borderColor: 'rgb(34, 197, 94)',
                backgroundColor: 'rgba(34, 197, 94, 0.1)',
                tension: 0.4,
                fill: true
            }]
        },
        options: {
            responsive: true,
            maintainAspectRatio: false,
            plugins: {
                legend: {
                    display: false
                }
            },
            scales: {
                animation: false, // PERFORMANCE: Disable animation to reduce lag
                y: {
                    grid: {
                        color: 'rgba(255, 255, 255, 0.1)'
                    },
                    ticks: {
                        color: 'rgba(255, 255, 255, 0.7)',
                        callback: function (value) {
                            return '$' + value.toFixed(2);
                        }
                    }
                },
                x: {
                    grid: {
                        color: 'rgba(255, 255, 255, 0.1)'
                    },
                    ticks: {
                        color: 'rgba(255, 255, 255, 0.7)',
                        maxTicksLimit: 8 // PERFORMANCE: Limit number of x-axis labels
                    }
                }
            }
        }
    });
}

// Fetch and update stats
async function updateStats() {
    try {
        const response = await fetch('/api/stats');
        const stats = await response.json();

        // Update stat cards
        document.getElementById('total-pnl').textContent = formatCurrency(stats.total_pnl);
        document.getElementById('total-pnl').className = stats.total_pnl >= 0 ?
            'text-3xl font-bold text-green-400' : 'text-3xl font-bold text-red-400';

        document.getElementById('win-rate').textContent = (stats.win_rate * 100).toFixed(1) + '%';
        document.getElementById('num-positions').textContent = stats.num_positions;
        document.getElementById('portfolio-value').textContent = formatCurrency(stats.portfolio_value);

        // Update chart
        const now = new Date().toLocaleTimeString();
        pnlHistory.push({ time: now, pnl: stats.total_pnl });

        // PERFORMANCE: Keep last 20 data points only (was 50)
        if (pnlHistory.length > 20) {
            pnlHistory.shift();
        }

        pnlChart.data.labels = pnlHistory.map(d => d.time);
        pnlChart.data.datasets[0].data = pnlHistory.map(d => d.pnl);
        pnlChart.update('none'); // PERFORMANCE: 'none' mode prevents re-animation

    } catch (error) {
        console.error('Error fetching stats:', error);
    }
}

// Fetch and update positions
async function updatePositions() {
    try {
        const response = await fetch('/api/positions');
        const positions = await response.json();

        const tbody = document.getElementById('positions-table');

        if (positions.length === 0) {
            tbody.innerHTML = '<tr><td colspan="6" class="text-center text-gray-500 py-8">No open positions</td></tr>';
            return;
        }

        tbody.innerHTML = positions.map(pos => {
            const pnl = (pos.current_price - pos.entry_price) * pos.size * (pos.side === 'YES' ? 1 : -1);
            const pnlClass = pnl >= 0 ? 'text-green-400' : 'text-red-400';

            return `
                <tr class="border-b border-gray-700">
                    <td class="py-3">${truncate(pos.market_question, 50)}</td>
                    <td class="py-3">
                        <span class="px-2 py-1 rounded text-xs ${pos.side === 'YES' ? 'bg-green-900 text-green-300' : 'bg-red-900 text-red-300'}">
                            ${pos.side}
                        </span>
                    </td>
                    <td class="py-3">${formatCurrency(pos.size)}</td>
                    <td class="py-3">${pos.entry_price.toFixed(4)}</td>
                    <td class="py-3">${pos.current_price.toFixed(4)}</td>
                    <td class="py-3 ${pnlClass}">${formatCurrency(pnl)}</td>
                </tr>
            `;
        }).join('');

    } catch (error) {
        console.error('Error fetching positions:', error);
    }
}

// Fetch and update trades
async function updateTrades() {
    try {
        const response = await fetch('/api/trades');
        const trades = await response.json();

        const tbody = document.getElementById('trades-table');

        if (trades.length === 0) {
            tbody.innerHTML = '<tr><td colspan="7" class="text-center text-gray-500 py-8">No trades yet</td></tr>';
            return;
        }

        // Show last 10 trades
        const recentTrades = trades.slice(-10).reverse();

        tbody.innerHTML = recentTrades.map(trade => {
            const pnl = trade.realized_pnl || 0;
            const pnlClass = pnl >= 0 ? 'text-green-400' : 'text-red-400';

            return `
                <tr class="border-b border-gray-700">
                    <td class="py-3">${new Date(trade.entry_time).toLocaleTimeString()}</td>
                    <td class="py-3">${truncate(trade.market_question, 40)}</td>
                    <td class="py-3">
                        <span class="px-2 py-1 rounded text-xs ${trade.side === 'YES' ? 'bg-green-900 text-green-300' : 'bg-red-900 text-red-300'}">
                            ${trade.side}
                        </span>
                    </td>
                    <td class="py-3">${formatCurrency(trade.size)}</td>
                    <td class="py-3">${trade.entry_price.toFixed(4)}</td>
                    <td class="py-3">${trade.exit_price ? trade.exit_price.toFixed(4) : '-'}</td>
                    <td class="py-3 ${pnlClass}">${trade.realized_pnl ? formatCurrency(pnl) : '-'}</td>
                </tr>
            `;
        }).join('');

    } catch (error) {
        console.error('Error fetching trades:', error);
    }
}

// Helper functions
function formatCurrency(value) {
    return '$' + value.toFixed(2);
}

function truncate(str, maxLen) {
    return str.length > maxLen ? str.substring(0, maxLen) + '...' : str;
}

// Initialize and start updates
initChart();
updateStats();
updatePositions();
updateTrades();

// Refresh every 5 seconds
setInterval(() => {
    updateStats();
    updatePositions();
    updateTrades();
}, 5000);
