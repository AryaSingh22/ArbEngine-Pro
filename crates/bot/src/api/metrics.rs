use crate::metrics::prometheus::MetricsCollector;
use axum::{response::IntoResponse, routing::get, Extension, Router};
use prometheus::{Encoder, TextEncoder};
use std::sync::Arc;

pub fn metrics_routes(metrics: Arc<MetricsCollector>) -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .layer(Extension(metrics))
}

async fn metrics_handler(
    Extension(metrics): Extension<Arc<MetricsCollector>>,
) -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = metrics.registry().gather();
    let mut buffer = Vec::new();

    // In a real app we might handle error better, but unwrap is safe for memory buffer
    encoder.encode(&metric_families, &mut buffer).unwrap();

    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        buffer,
    )
}
