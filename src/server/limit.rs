use axum::extract::Request;
use axum::http::{Response, StatusCode};
use governor::middleware::NoOpMiddleware;
use metrics::counter;
use std::sync::Arc;
use tower_governor::{
    GovernorLayer, errors::GovernorError, governor::GovernorConfigBuilder,
    key_extractor::KeyExtractor,
};
use tracing::{debug, warn};

/// Custom key extractor that tries to get IP from various headers and falls back to a default
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RobustIpKeyExtractor;

impl KeyExtractor for RobustIpKeyExtractor {
    type Key = String;

    fn extract<B>(&self, req: &Request<B>) -> Result<Self::Key, GovernorError> {
        // Output debugging information
        debug!(
            headers = ?req.headers(),
            "Attempting to extract IP address from request"
        );
        // Try to extract IP from various headers in order of preference
        let ip = req
            .headers()
            .get("Authorization")
            .and_then(|token| token.to_str().ok())
            .and_then(|token| token.strip_prefix("Bearer "))
            .map(|token| token.trim())
            .or_else(|| {
                req.headers()
                    .get("X-Forwarded-For")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.split(',').next())
                    .map(|s| s.trim())
            })
            .or_else(|| {
                req.headers()
                    .get("X-Real-IP") // Nginx
                    .and_then(|h| h.to_str().ok())
            })
            .or_else(|| {
                req.headers()
                    .get("X-Client-IP") // Proxies
                    .and_then(|h| h.to_str().ok())
            })
            .or_else(|| {
                req.headers()
                    .get("CF-Connecting-IP") // Cloudflare
                    .and_then(|h| h.to_str().ok())
            })
            .or_else(|| {
                req.headers()
                    .get("True-Client-IP") // Akamai
                    .and_then(|h| h.to_str().ok())
            })
            .or_else(|| {
                req.headers()
                    .get("X-Originating-IP")
                    .and_then(|h| h.to_str().ok())
            })
            .or_else(|| {
                req.headers()
                    .get("X-Remote-IP")
                    .and_then(|h| h.to_str().ok())
            })
            .or_else(|| {
                req.headers()
                    .get("X-Remote-Addr")
                    .and_then(|h| h.to_str().ok())
            });
        // If we find an idenfitying key, use it
        if let Some(ip) = ip {
            debug!(ip = ip, "Extracted IP address from headers");
            return Ok(ip.to_string());
        }
        // Otherwise, try to retrieve the connection info
        if let Some(addr) = req.extensions().get::<std::net::SocketAddr>() {
            debug!(ip = ?addr.ip(), "Extracted IP address from socket");
            return Ok(addr.ip().to_string());
        }
        // If we don't find an identifying key, use a default key
        warn!("Could not extract IP address from request, using default key");
        Ok("unknown".to_string())
    }
}

/// Create a rate limiting layer with metrics and logging
pub fn create_rate_limit_layer(
    rps: u32,
    burst: u32,
) -> GovernorLayer<RobustIpKeyExtractor, NoOpMiddleware> {
    // Output debugging information
    debug!("Configuring the HTTP rate limiter");
    // Create the rate limit configuration
    let config = GovernorConfigBuilder::default()
        .per_second(rps as u64)
        .burst_size(burst)
        .key_extractor(RobustIpKeyExtractor)
        .error_handler(|e| {
            // Output debugging information
            warn!("Rate limit exceeded: {e}");
            // Increment rate limit error metrics
            counter!("surrealmcp.total_errors").increment(1);
            counter!("surrealmcp.total_rate_limit_errors").increment(1);
            // Return the error response
            Response::builder()
                .status(StatusCode::TOO_MANY_REQUESTS)
                .body("Rate limit exceeded".into())
                .unwrap()
        })
        .finish()
        .expect("Failed to create rate limit configuration");
    // Return the rate limit layer
    GovernorLayer::<RobustIpKeyExtractor, NoOpMiddleware> {
        config: Arc::new(config),
    }
}
