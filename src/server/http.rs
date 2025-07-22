use axum::http::StatusCode;

/// Health check endpoint for load balancer health status checking
pub async fn health() -> StatusCode {
    StatusCode::OK
}
