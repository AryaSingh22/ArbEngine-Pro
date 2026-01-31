import { useState, useEffect } from 'react';
import { api } from '../api';
import './StatusBar.css';

export function StatusBar() {
    const [status, setStatus] = useState<'connected' | 'disconnected' | 'checking'>('checking');
    const [version, setVersion] = useState<string>('');
    const [lastUpdate, setLastUpdate] = useState<Date>(new Date());

    const checkHealth = async () => {
        const response = await api.health();
        if (response.success && response.data) {
            setStatus('connected');
            setVersion(response.data.version || 'unknown');
            setLastUpdate(new Date());
        } else {
            setStatus('disconnected');
        }
    };

    useEffect(() => {
        checkHealth();
        const interval = setInterval(checkHealth, 5000);
        return () => clearInterval(interval);
    }, []);

    return (
        <div className="status-bar">
            <div className="status-item">
                <span className={`status-dot ${status}`}></span>
                <span className="status-text">
                    {status === 'connected' ? 'API Connected' :
                        status === 'disconnected' ? 'API Disconnected' : 'Checking...'}
                </span>
            </div>

            {version && (
                <div className="status-item">
                    <span className="version">v{version}</span>
                </div>
            )}

            <div className="status-item">
                <span className="last-update">
                    Last update: {lastUpdate.toLocaleTimeString()}
                </span>
            </div>
        </div>
    );
}
