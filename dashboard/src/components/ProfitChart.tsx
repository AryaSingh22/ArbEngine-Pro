import { useState, useEffect } from 'react';
import {
    AreaChart,
    Area,
    XAxis,
    YAxis,
    CartesianGrid,
    Tooltip,
    ResponsiveContainer,
    Legend,
} from 'recharts';
import type { ArbitrageOpportunity } from '../types';
import { api } from '../api';
import './ProfitChart.css';

interface ChartDataPoint {
    time: string;
    profit: number;
    count: number;
}

export function ProfitChart() {
    const [chartData, setChartData] = useState<ChartDataPoint[]>([]);
    const [timeRange, setTimeRange] = useState<'1m' | '5m' | '15m'>('5m');

    useEffect(() => {
        const fetchData = async () => {
            const response = await api.getOpportunities({ limit: 100 });
            if (response.success && response.data) {
                const aggregated = aggregateByMinute(response.data, timeRange);
                setChartData(aggregated);
            }
        };

        fetchData();
        const interval = setInterval(fetchData, 5000);
        return () => clearInterval(interval);
    }, [timeRange]);

    const aggregateByMinute = (
        opportunities: ArbitrageOpportunity[],
        range: string
    ): ChartDataPoint[] => {
        const now = new Date();
        const minutes = range === '1m' ? 1 : range === '5m' ? 5 : 15;
        const buckets: Map<string, { profit: number; count: number }> = new Map();

        // Initialize buckets
        for (let i = minutes; i >= 0; i--) {
            const time = new Date(now.getTime() - i * 60000);
            const key = time.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
            buckets.set(key, { profit: 0, count: 0 });
        }

        // Aggregate opportunities
        opportunities.forEach((opp) => {
            const oppTime = new Date(opp.detected_at);
            const key = oppTime.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
            if (buckets.has(key)) {
                const bucket = buckets.get(key)!;
                bucket.profit += parseFloat(opp.net_profit_pct);
                bucket.count += 1;
            }
        });

        return Array.from(buckets.entries()).map(([time, data]) => ({
            time,
            profit: Number(data.profit.toFixed(4)),
            count: data.count,
        }));
    };

    const CustomTooltip = ({ active, payload, label }: any) => {
        if (active && payload && payload.length) {
            return (
                <div className="chart-tooltip">
                    <p className="tooltip-time">{label}</p>
                    <p className="tooltip-profit">
                        Avg Profit: <span>+{payload[0].value.toFixed(4)}%</span>
                    </p>
                    <p className="tooltip-count">
                        Opportunities: <span>{payload[1]?.value || 0}</span>
                    </p>
                </div>
            );
        }
        return null;
    };

    return (
        <div className="profit-chart-container">
            <div className="chart-header">
                <h2>📈 Profit Trend</h2>
                <div className="time-selector">
                    {(['1m', '5m', '15m'] as const).map((range) => (
                        <button
                            key={range}
                            className={`time-btn ${timeRange === range ? 'active' : ''}`}
                            onClick={() => setTimeRange(range)}
                        >
                            {range}
                        </button>
                    ))}
                </div>
            </div>

            <div className="chart-wrapper">
                <ResponsiveContainer width="100%" height={300}>
                    <AreaChart data={chartData} margin={{ top: 10, right: 30, left: 0, bottom: 0 }}>
                        <defs>
                            <linearGradient id="profitGradient" x1="0" y1="0" x2="0" y2="1">
                                <stop offset="5%" stopColor="#4ade80" stopOpacity={0.3} />
                                <stop offset="95%" stopColor="#4ade80" stopOpacity={0} />
                            </linearGradient>
                            <linearGradient id="countGradient" x1="0" y1="0" x2="0" y2="1">
                                <stop offset="5%" stopColor="#667eea" stopOpacity={0.3} />
                                <stop offset="95%" stopColor="#667eea" stopOpacity={0} />
                            </linearGradient>
                        </defs>
                        <CartesianGrid strokeDasharray="3 3" stroke="rgba(255,255,255,0.1)" />
                        <XAxis dataKey="time" stroke="#718096" fontSize={12} />
                        <YAxis stroke="#718096" fontSize={12} />
                        <Tooltip content={<CustomTooltip />} />
                        <Legend />
                        <Area
                            type="monotone"
                            dataKey="profit"
                            name="Profit %"
                            stroke="#4ade80"
                            fill="url(#profitGradient)"
                            strokeWidth={2}
                        />
                        <Area
                            type="monotone"
                            dataKey="count"
                            name="Count"
                            stroke="#667eea"
                            fill="url(#countGradient)"
                            strokeWidth={2}
                        />
                    </AreaChart>
                </ResponsiveContainer>
            </div>
        </div>
    );
}
