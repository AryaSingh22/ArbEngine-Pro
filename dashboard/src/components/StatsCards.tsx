import { useState, useEffect } from 'react';
import { api } from '../api';
import './StatsCards.css';

interface Stats {
    totalOpportunities: number;
    avgProfit: number;
    bestProfit: number;
    activeDexs: number;
    totalVolume: number;
}

export function StatsCards() {
    const [stats, setStats] = useState<Stats>({
        totalOpportunities: 0,
        avgProfit: 0,
        bestProfit: 0,
        activeDexs: 0,
        totalVolume: 0,
    });

    useEffect(() => {
        const fetchStats = async () => {
            const [oppResponse, priceResponse] = await Promise.all([
                api.getOpportunities({ limit: 50 }),
                api.getPrices(),
            ]);

            if (oppResponse.success && oppResponse.data) {
                const opportunities = oppResponse.data;
                const profits = opportunities.map((o) => parseFloat(o.net_profit_pct));

                const totalOpportunities = opportunities.length;
                const avgProfit = profits.length > 0
                    ? profits.reduce((a, b) => a + b, 0) / profits.length
                    : 0;
                const bestProfit = profits.length > 0 ? Math.max(...profits) : 0;

                let activeDexs = 0;
                let totalVolume = 0;

                if (priceResponse.success && priceResponse.data) {
                    const dexSet = new Set(priceResponse.data.map((p) => p.dex));
                    activeDexs = dexSet.size;
                    totalVolume = priceResponse.data.reduce((sum, p) => {
                        return sum + (p.volume_24h ? parseFloat(p.volume_24h) : 0);
                    }, 0);
                }

                setStats({
                    totalOpportunities,
                    avgProfit,
                    bestProfit,
                    activeDexs,
                    totalVolume,
                });
            }
        };

        fetchStats();
        const interval = setInterval(fetchStats, 3000);
        return () => clearInterval(interval);
    }, []);

    const formatNumber = (num: number) => {
        if (num >= 1_000_000) return (num / 1_000_000).toFixed(2) + 'M';
        if (num >= 1_000) return (num / 1_000).toFixed(2) + 'K';
        return num.toFixed(2);
    };

    return (
        <div className="stats-grid">
            <div className="stat-card">
                <div className="stat-icon">🎯</div>
                <div className="stat-content">
                    <span className="stat-label">Active Opportunities</span>
                    <span className="stat-value">{stats.totalOpportunities}</span>
                </div>
            </div>

            <div className="stat-card highlight">
                <div className="stat-icon">💰</div>
                <div className="stat-content">
                    <span className="stat-label">Avg Profit</span>
                    <span className="stat-value profit">+{stats.avgProfit.toFixed(4)}%</span>
                </div>
            </div>

            <div className="stat-card">
                <div className="stat-icon">🚀</div>
                <div className="stat-content">
                    <span className="stat-label">Best Opportunity</span>
                    <span className="stat-value best">+{stats.bestProfit.toFixed(4)}%</span>
                </div>
            </div>

            <div className="stat-card">
                <div className="stat-icon">🔗</div>
                <div className="stat-content">
                    <span className="stat-label">Active DEXs</span>
                    <span className="stat-value">{stats.activeDexs}</span>
                </div>
            </div>

            <div className="stat-card wide">
                <div className="stat-icon">📊</div>
                <div className="stat-content">
                    <span className="stat-label">24h Volume (Combined)</span>
                    <span className="stat-value">${formatNumber(stats.totalVolume)}</span>
                </div>
            </div>
        </div>
    );
}
