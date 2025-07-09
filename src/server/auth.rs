use axum::http::Request;
use axum::middleware::Next;
use axum::{
    http::StatusCode,
    http::header::{AUTHORIZATION, WWW_AUTHENTICATE},
    response::{IntoResponse, Response},
};

const WWW_AUTHENTICATE_VALUE: &str =
    "Bearer resource_metadata='/.well-known/oauth-protected-resource'";

// This middleware checks for a Bearer token and validates it.
pub async fn require_bearer_auth(
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Get the current request path
    let path = req.uri().path();
    // Allow access to auth metadata
    if path.starts_with("/.well-known/") {
        return Ok(next.run(req).await);
    }
    // Check for an Authorization header
    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok());
    // If the header is present, check the token
    if let Some(auth_header) = auth_header {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            // TODO: Validate the token (signature, expiry, audience, etc.)
            if !token.trim().is_empty() {
                return Ok(next.run(req).await);
            }
        }
    }
    // If missing or invalid, return 401 with detailed WWW-Authenticate header only
    let res = (
        StatusCode::UNAUTHORIZED,
        [(WWW_AUTHENTICATE, WWW_AUTHENTICATE_VALUE)],
    );
    let res = res.into_response();
    // Return the 401 response
    Ok(res)
}
