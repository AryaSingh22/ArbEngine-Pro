// API types matching the Rust backend

export type DexType = 'raydium' | 'orca' | 'jupiter';

export interface TokenPair {
    base: string;
    quote: string;
}

export interface PriceData {
    dex: DexType;
    pair: TokenPair;
    bid: string;
    ask: string;
    mid_price: string;
    volume_24h?: string;
    liquidity?: string;
    timestamp: string;
}

export interface ArbitrageOpportunity {
    id: string;
    pair: TokenPair;
    buy_dex: DexType;
    sell_dex: DexType;
    buy_price: string;
    sell_price: string;
    gross_profit_pct: string;
    net_profit_pct: string;
    estimated_profit_usd?: string;
    recommended_size?: string;
    detected_at: string;
    expired_at?: string;
}

export interface ApiResponse<T> {
    success: boolean;
    data?: T;
    error?: string;
}

export interface Config {
    min_profit_threshold: number;
    api_port: number;
    log_level: string;
}
