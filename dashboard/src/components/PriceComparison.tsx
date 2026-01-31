import { useState, useEffect } from 'react';
import type { PriceData } from '../types';
import { api } from '../api';
import './PriceComparison.css';

const PAIRS = ['SOL-USDC', 'RAY-USDC', 'ORCA-USDC', 'JUP-USDC'];

export function PriceComparison() {
    const [prices, setPrices] = useState<PriceData[]>([]);
    const [loading, setLoading] = useState(true);
    const [selectedPair, setSelectedPair] = useState(PAIRS[0]);

    const fetchPrices = async () => {
        const response = await api.getPrices();
        if (response.success && response.data) {
            setPrices(response.data);
        }
        setLoading(false);
    };

    useEffect(() => {
        fetchPrices();
        const interval = setInterval(fetchPrices, 1000);
        return () => clearInterval(interval);
    }, []);

    const formatPrice = (value: string) => {
        const num = parseFloat(value);
        return '$' + num.toLocaleString('en-US', { minimumFractionDigits: 4, maximumFractionDigits: 6 });
    };

    const formatSpread = (bid: string, ask: string) => {
        const b = parseFloat(bid);
        const a = parseFloat(ask);
        const spread = ((a - b) / ((a + b) / 2)) * 100;
        return spread.toFixed(4) + '%';
    };

    const getPricesForPair = (pair: string) => {
        const [base, quote] = pair.split('-');
        return prices.filter(p => p.pair.base === base && p.pair.quote === quote);
    };

    const getDexIcon = (dex: string) => {
        switch (dex.toLowerCase()) {
            case 'raydium': return '🌱';
            case 'orca': return '🐋';
            case 'jupiter': return '🪐';
            default: return '📊';
        }
    };

    const pairPrices = getPricesForPair(selectedPair);

    return (
        <div className="price-comparison-container">
            <div className="price-header">
                <h2>📊 Live Price Comparison</h2>
                <div className="pair-selector">
                    {PAIRS.map(pair => (
                        <button
                            key={pair}
                            className={`pair-btn ${selectedPair === pair ? 'active' : ''}`}
                            onClick={() => setSelectedPair(pair)}
                        >
                            {pair.replace('-', '/')}
                        </button>
                    ))}
                </div>
            </div>

            {loading ? (
                <div className="price-loading">
                    <div className="spinner"></div>
                    <p>Loading prices...</p>
                </div>
            ) : pairPrices.length === 0 ? (
                <div className="no-prices">
                    <p>No price data available for {selectedPair}</p>
                </div>
            ) : (
                <div className="price-grid">
                    {pairPrices.map((price, idx) => (
                        <div key={idx} className={`price-card dex-card-${price.dex.toLowerCase()}`}>
                            <div className="dex-header">
                                <span className="dex-icon">{getDexIcon(price.dex)}</span>
                                <span className="dex-name">{price.dex}</span>
                            </div>

                            <div className="price-main">
                                <span className="mid-price">{formatPrice(price.mid_price)}</span>
                            </div>

                            <div className="price-details">
                                <div className="price-row">
                                    <span className="label">Bid</span>
                                    <span className="value bid">{formatPrice(price.bid)}</span>
                                </div>
                                <div className="price-row">
                                    <span className="label">Ask</span>
                                    <span className="value ask">{formatPrice(price.ask)}</span>
                                </div>
                                <div className="price-row">
                                    <span className="label">Spread</span>
                                    <span className="value spread">{formatSpread(price.bid, price.ask)}</span>
                                </div>
                            </div>

                            {price.volume_24h && (
                                <div className="volume">
                                    24h Vol: ${parseFloat(price.volume_24h).toLocaleString()}
                                </div>
                            )}
                        </div>
                    ))}
                </div>
            )}
        </div>
    );
}
