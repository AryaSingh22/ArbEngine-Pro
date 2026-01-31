import { useState, useEffect } from 'react';
import type { ArbitrageOpportunity } from '../types';
import { api } from '../api';
import './OpportunitiesTable.css';

export function OpportunitiesTable() {
    const [opportunities, setOpportunities] = useState<ArbitrageOpportunity[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    const fetchOpportunities = async () => {
        const response = await api.getOpportunities({ limit: 20 });
        if (response.success && response.data) {
            setOpportunities(response.data);
            setError(null);
        } else {
            setError(response.error || 'Failed to fetch opportunities');
        }
        setLoading(false);
    };

    useEffect(() => {
        fetchOpportunities();
        const interval = setInterval(fetchOpportunities, 1000);
        return () => clearInterval(interval);
    }, []);

    const formatPercent = (value: string) => {
        const num = parseFloat(value);
        return num.toFixed(4) + '%';
    };

    const formatPrice = (value: string) => {
        const num = parseFloat(value);
        return num.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 6 });
    };

    const getDexClass = (dex: string) => {
        return `dex-badge dex-${dex.toLowerCase()}`;
    };

    const getProfitClass = (profit: string) => {
        const num = parseFloat(profit);
        if (num >= 1) return 'profit-high';
        if (num >= 0.5) return 'profit-medium';
        return 'profit-low';
    };

    if (loading) {
        return (
            <div className="opportunities-loading">
                <div className="spinner"></div>
                <p>Loading opportunities...</p>
            </div>
        );
    }

    if (error) {
        return (
            <div className="opportunities-error">
                <p>⚠️ {error}</p>
                <button onClick={fetchOpportunities}>Retry</button>
            </div>
        );
    }

    return (
        <div className="opportunities-container">
            <div className="opportunities-header">
                <h2>🔥 Live Arbitrage Opportunities</h2>
                <span className="opportunity-count">{opportunities.length} found</span>
            </div>

            {opportunities.length === 0 ? (
                <div className="no-opportunities">
                    <p>No arbitrage opportunities detected at this time.</p>
                    <p className="hint">Opportunities appear when price differences exceed the minimum threshold.</p>
                </div>
            ) : (
                <table className="opportunities-table">
                    <thead>
                        <tr>
                            <th>Pair</th>
                            <th>Buy From</th>
                            <th>Buy Price</th>
                            <th>Sell To</th>
                            <th>Sell Price</th>
                            <th>Net Profit</th>
                            <th>Detected</th>
                        </tr>
                    </thead>
                    <tbody>
                        {opportunities.map((opp) => (
                            <tr key={opp.id} className={getProfitClass(opp.net_profit_pct)}>
                                <td className="pair-cell">
                                    <span className="pair-symbol">{opp.pair.base}/{opp.pair.quote}</span>
                                </td>
                                <td>
                                    <span className={getDexClass(opp.buy_dex)}>{opp.buy_dex}</span>
                                </td>
                                <td className="price-cell">${formatPrice(opp.buy_price)}</td>
                                <td>
                                    <span className={getDexClass(opp.sell_dex)}>{opp.sell_dex}</span>
                                </td>
                                <td className="price-cell">${formatPrice(opp.sell_price)}</td>
                                <td className="profit-cell">
                                    <span className="profit-value">+{formatPercent(opp.net_profit_pct)}</span>
                                </td>
                                <td className="time-cell">
                                    {new Date(opp.detected_at).toLocaleTimeString()}
                                </td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            )}
        </div>
    );
}
