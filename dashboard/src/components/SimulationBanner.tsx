import { useState, useEffect } from 'react';
import './SimulationBanner.css';

interface DexHealth {
    name: string;
    last_success_at: string | null;
    consecutive_errors: number;
    status: string;
}

interface StatusData {
    dry_run: boolean;
    bot_running: boolean;
    simulated_pnl: number;
    simulated_trades: number;
    heartbeat_count: number;
    last_scan_at: string;
    dex_health: DexHealth[];
}

function getRelativeTime(isoString: string): string {
    const now = new Date();
    const then = new Date(isoString);
    const diffMs = now.getTime() - then.getTime();
    const diffSecs = Math.floor(diffMs / 1000);

    if (diffSecs < 5) return 'just now';
    if (diffSecs < 60) return `${diffSecs} seconds ago`;
    if (diffSecs < 120) return '1 minute ago';
    return `${Math.floor(diffSecs / 60)} minutes ago`;
}

function getStatusColor(status: string): string {
    switch (status) {
        case 'green': return '#22c55e';
        case 'yellow': return '#eab308';
        case 'red': return '#ef4444';
        default: return '#6b7280';
    }
}

export function SimulationBanner() {
    const [status, setStatus] = useState<StatusData | null>(null);

    useEffect(() => {
        const fetchStatus = async () => {
            try {
                const res = await fetch('/api/status');
                const data = await res.json();
                if (data.success) {
                    setStatus(data.data);
                }
            } catch (err) {
                console.error('Failed to fetch status:', err);
            }
        };

        fetchStatus();
        const interval = setInterval(fetchStatus, 2000); // Poll every 2s for liveness
        return () => clearInterval(interval);
    }, []);

    if (!status) {
        return null;
    }

    return (
        <div className={`simulation-banner ${status.dry_run ? 'dry-run' : 'live'}`}>
            <div className="simulation-banner-content">
                {/* Mode Indicator */}
                <div className="mode-section">
                    <span className="simulation-icon">{status.dry_run ? '🧪' : '🟢'}</span>
                    <span className="simulation-text">
                        <strong>{status.dry_run ? 'SIMULATION MODE' : 'LIVE MODE'}</strong>
                        {status.dry_run && ' — No real trades'}
                    </span>
                </div>

                {/* Liveness Stats */}
                <div className="liveness-section">
                    <span className="stat heartbeat">
                        💓 <strong>{status.heartbeat_count.toLocaleString()}</strong> scans
                    </span>
                    <span className="stat last-update">
                        🕐 {getRelativeTime(status.last_scan_at)}
                    </span>
                </div>

                {/* DEX Health Indicators */}
                <div className="dex-health-section">
                    {status.dex_health?.map((dex) => (
                        <span
                            key={dex.name}
                            className="dex-indicator"
                            title={`${dex.name}: ${dex.status}`}
                        >
                            <span
                                className="dex-dot"
                                style={{ backgroundColor: getStatusColor(dex.status) }}
                            />
                            {dex.name.replace('DexType::', '')}
                        </span>
                    ))}
                </div>

                {/* Trade Stats (if dry run) */}
                {status.dry_run && (
                    <div className="simulation-stats">
                        <span className="stat">
                            P&L: <strong>${status.simulated_pnl.toFixed(2)}</strong>
                        </span>
                        <span className="stat">
                            Trades: <strong>{status.simulated_trades}</strong>
                        </span>
                    </div>
                )}
            </div>
        </div>
    );
}
