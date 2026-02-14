use reqwest::{Client, ClientBuilder};
use std::time::Duration;

pub fn create_optimized_client() -> Client {
    ClientBuilder::new()
        .pool_max_idle_per_host(50) // Keep connections alive
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_keepalive(Duration::from_secs(60))
        .tcp_nodelay(true) // Disable Nagle's algorithm
        .timeout(Duration::from_millis(500))
        .build()
        .expect("Failed to create HTTP client")
}
