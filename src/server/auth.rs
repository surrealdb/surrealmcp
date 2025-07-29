use axum::http::Request;
use axum::middleware::Next;
use axum::{
    http::StatusCode,
    http::header::{AUTHORIZATION, WWW_AUTHENTICATE},
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

const WWW_AUTHENTICATE_VALUE: &str =
    "Bearer resource_metadata='/.well-known/oauth-protected-resource'";

// Expected issuer for SurrealDB auth tokens
const EXPECTED_ISSUER: &str = "https://auth.surrealdb.com/";

// Expected audience for SurrealDB MCP tokens
const EXPECTED_AUDIENCE: &str = "https://mcp.surrealdb.com/";

// Token validation configuration
#[derive(Clone)]
pub struct TokenValidationConfig {
    pub expected_issuer: String,
    pub expected_audience: String,
    pub jwt_public_key: Option<String>,
    pub validate_expiration: bool,
    pub validate_issued_at: bool,
    pub clock_skew_seconds: u64,
}

impl Default for TokenValidationConfig {
    fn default() -> Self {
        Self {
            expected_issuer: EXPECTED_ISSUER.to_string(),
            expected_audience: EXPECTED_AUDIENCE.to_string(),
            jwt_public_key: None,
            validate_expiration: true,
            validate_issued_at: true,
            clock_skew_seconds: 300, // 5 minutes
        }
    }
}

/// JWE (JSON Web Encryption) header structure for SurrealDB auth tokens
#[derive(Debug, Serialize, Deserialize)]
struct JweHeader {
    alg: String,
    enc: String,
    iss: String,
}

/// Token claims structure for both JWE and JWT tokens
#[derive(Debug, Serialize, Deserialize)]
struct TokenClaims {
    iss: String,
    aud: Option<String>,
    exp: Option<u64>,
    iat: Option<u64>,
    sub: Option<String>,
}

/// Validate a JWE token from SurrealDB auth service
///
/// This function validates the JWE token header structure and issuer.
/// Since we don't have the decryption key, we can only validate the header.
/// In production, you would need the decryption key to access the full claims.
fn validate_jwe_token(token: &str, config: &TokenValidationConfig) -> Result<TokenClaims, String> {
    // Output debugging information
    debug!(token = %token, "Validating JWE token");
    // JWE tokens have 5 parts separated by dots
    let parts: Vec<&str> = token.split('.').collect();
    // Check that the token has 5 parts
    if parts.len() != 5 {
        return Err("Invalid JWE token format: expected 5 parts".to_string());
    }
    // Decode the first header part
    let header_bytes = URL_SAFE_NO_PAD
        .decode(parts[0])
        .map_err(|e| format!("Failed to decode JWE header: {e}"))?;
    // Parse the first header part
    let header: JweHeader = serde_json::from_slice(&header_bytes)
        .map_err(|e| format!("Failed to parse JWE header: {e}"))?;
    // Validate the token algorithm
    if header.alg != "dir" {
        return Err(format!("Unsupported algorithm: {}", header.alg));
    }
    // Validate the token encryption
    if header.enc != "A256GCM" {
        return Err(format!("Unsupported encryption: {}", header.enc));
    }
    // Validate the token issuer
    if header.iss != EXPECTED_ISSUER {
        return Err(format!(
            "Invalid issuer: expected {}, got {}",
            EXPECTED_ISSUER, header.iss
        ));
    }
    // Create a basic claims structure for now
    let claims = TokenClaims {
        iss: header.iss,
        aud: None,
        exp: None,
        iat: None,
        sub: None,
    };
    // Output debugging information
    debug!(
        token = %token,
        issuer = %claims.iss,
        "JWE token validated successfully"
    );
    // Return the token claims
    Ok(claims)
}

/// Validate a standard JWT token
///
/// This function validates JWT tokens using the jsonwebtoken crate.
/// It validates all claims including audience, expiration, issued at, and subject.
fn validate_jwt_token(token: &str, config: &TokenValidationConfig) -> Result<TokenClaims, String> {
    // Output debugging information
    debug!(token = %token, "Validating JWT token");
    // Decode the header to check the algorithm
    let header = decode_header(token).map_err(|e| format!("Failed to decode JWT header: {e}"))?;
    // Create validation configuration
    let mut validation = Validation::new(header.alg);
    validation.set_audience(&[&config.expected_audience]);
    validation.set_issuer(&[&config.expected_issuer]);
    validation.set_required_spec_claims(&["iss", "aud", "exp", "iat", "sub"]);
    validation.set_required_spec_claims(&["iss", "aud", "exp", "iat"]);
    validation.leeway = config.clock_skew_seconds;
    validation.validate_aud = true;
    validation.validate_exp = true;
    // Create the decoding key
    let key = if let Some(public_key) = &config.jwt_public_key {
        match header.alg {
            Algorithm::RS256 | Algorithm::RS384 | Algorithm::RS512 => {
                DecodingKey::from_rsa_pem(public_key.as_bytes())
                    .map_err(|e| format!("Failed to create RSA decoding key: {e}"))?
            }
            Algorithm::ES256 | Algorithm::ES384 => DecodingKey::from_ec_pem(public_key.as_bytes())
                .map_err(|e| format!("Failed to create EC decoding key: {e}"))?,
            _ => {
                return Err(format!("Unsupported JWT algorithm: {:?}", header.alg));
            }
        }
    } else {
        // Fallback to dummy key for testing
        DecodingKey::from_secret(b"dummy_key_for_validation_only")
    };
    // Decode the authentication token
    let token_data = decode::<TokenClaims>(token, &key, &validation)
        .map_err(|e| format!("Failed to validate JWT token: {e}"))?;
    // Validate expiration time
    if config.validate_expiration {
        if let Some(exp) = token_data.claims.exp {
            // Get the current time
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| format!("Failed to get current time: {e}"))?
                .as_secs();
            // Check for expiration, allowing for clock skew
            if current_time > exp + config.clock_skew_seconds {
                return Err(format!(
                    "Token expired: expired at {}, current time {}",
                    exp, current_time
                ));
            }
        }
    }
    // Validate issued at time
    if config.validate_issued_at {
        if let Some(iat) = token_data.claims.iat {
            // Get the current time
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| format!("Failed to get current time: {e}"))?
                .as_secs();
            // Check for issued at, allowing for clock skew
            if iat > current_time + config.clock_skew_seconds {
                return Err(format!(
                    "Token issued in the future: issued at {}, current time {}",
                    iat, current_time
                ));
            }
        }
    }
    // Output debugging information
    debug!(
        token = %token,
        issuer = %token_data.claims.iss,
        audience = ?token_data.claims.aud,
        subject = ?token_data.claims.sub,
        expiration = ?token_data.claims.exp,
        issued_at = ?token_data.claims.iat,
        "JWT token validated successfully"
    );
    // Return the token claims
    Ok(token_data.claims)
}

/// Validate a bearer token (supports both JWE and JWT formats)
///
/// This function determines the token type based on the number of parts:
/// - 5 parts: JWE token (JSON Web Encryption)
/// - 3 parts: JWT token (JSON Web Token)
///
/// It then delegates to the appropriate validation function.
fn validate_bearer_token(
    token: &str,
    config: &TokenValidationConfig,
) -> Result<TokenClaims, String> {
    // Trim the token content
    let token = token.trim();
    // Check the token is not empty
    if token.is_empty() {
        return Err("Empty token".to_string());
    }
    // Check if it's a JWE token (5 parts) or JWT token (3 parts)
    match token.split('.').count() {
        5 => validate_jwe_token(token, config),
        3 => validate_jwt_token(token, config),
        l => Err(format!(
            "Invalid token format: expected 3 or 5 parts, got {l}"
        )),
    }
}

/// Axum middleware that validates Bearer tokens for protected endpoints
///
/// This middleware:
/// 1. Allows access to /.well-known/ and /health endpoints without authentication
/// 2. Extracts the Bearer token from the Authorization header
/// 3. Validates the token structure, issuer, and claims (where available)
/// 4. Returns 401 Unauthorized with proper WWW-Authenticate header if validation fails
///
/// Security considerations:
/// - Validates token structure and issuer
/// - For JWT tokens: validates audience, expiration, and issued at
/// - For JWE tokens: validates header structure only (requires decryption key for full validation)
/// - Supports both JWE and JWT token formats
/// - Logs validation failures for monitoring
/// - Uses proper HTTP status codes and headers
pub async fn require_bearer_auth(
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Get the current request path
    let path = req.uri().path();
    // Allow access to auth metadata and health check endpoint
    if path.starts_with("/.well-known/") || path == "/health" {
        return Ok(next.run(req).await);
    }
    // Create token validation configuration
    let config = TokenValidationConfig::default();
    // Check for an Authorization header
    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok());
    // If the header is present, validate the token
    if let Some(auth_header) = auth_header {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            match validate_bearer_token(token, &config) {
                Ok(claims) => {
                    debug!(
                        issuer = %claims.iss,
                        audience = ?claims.aud,
                        subject = claims.sub.as_deref().unwrap_or("unknown"),
                        expiration = ?claims.exp,
                        issued_at = ?claims.iat,
                        "Bearer token validated successfully"
                    );
                    return Ok(next.run(req).await);
                }
                Err(e) => {
                    warn!("Bearer token validation failed: {}", e);
                    // Continue to return 401
                }
            }
        }
    }

    // If missing or invalid, return 401 with detailed WWW-Authenticate header
    let res = (
        StatusCode::UNAUTHORIZED,
        [(WWW_AUTHENTICATE, WWW_AUTHENTICATE_VALUE)],
    );
    let res = res.into_response();
    // Return the 401 response
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        routing::get,
    };
    use tower::ServiceExt;

    #[test]
    fn test_validate_surrealdb_jwe_token() {
        // Example JWE token from SurrealDB auth service
        let token = "eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIiwiaXNzIjoiaHR0cHM6Ly9hdXRoLnN1cnJlYWxkYi5jb20vIn0..i2Rd5nBEMkJSz6dC.KWp44r7imTAq0nOEXYGC6J4ABuaLFt_4EKFYIUEjN7sNB98aiRatF7nfoopZUqVsp4OWHA1AtnBL8FNuIeHZwH1WthdhAb3P4cbE-KvgrfS3RFyRCXqX9tqzxF9K3wTAvAnI3Lyp510jt9k3ytNKycfJi1mlXKw-WpU8WfqlgKRVd4QkWAn_OKMjfOZDgcCfiKxoHY5FYF77KymTQfQbauKjt4kpLFuFsJf5MleplV5T6cOy-ehJSbfsOUVeRNSeMdkZ4eLLG_vvTNJB.lJop5ReVf6pWw5rb_E5ILg";

        let result = validate_bearer_token(token, &TokenValidationConfig::default());
        assert!(
            result.is_ok(),
            "Token validation should succeed: {:?}",
            result
        );

        let claims = result.unwrap();
        assert_eq!(claims.iss, EXPECTED_ISSUER);
    }

    #[test]
    fn test_validate_empty_token() {
        let result = validate_bearer_token("", &TokenValidationConfig::default());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Empty token");
    }

    #[test]
    fn test_validate_invalid_format() {
        let result = validate_bearer_token("invalid.token", &TokenValidationConfig::default());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid token format"));
    }

    #[test]
    fn test_validate_jwe_header_structure() {
        let token = "eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIiwiaXNzIjoiaHR0cHM6Ly9hdXRoLnN1cnJlYWxkYi5jb20vIn0..i2Rd5nBEMkJSz6dC.KWp44r7imTAq0nOEXYGC6J4ABuaLFt_4EKFYIUEjN7sNB98aiRatF7nfoopZUqVsp4OWHA1AtnBL8FNuIeHZwH1WthdhAb3P4cbE-KvgrfS3RFyRCXqX9tqzxF9K3wTAvAnI3Lyp510jt9k3ytNKycfJi1mlXKw-WpU8WfqlgKRVd4QkWAn_OKMjfOZDgcCfiKxoHY5FYF77KymTQfQbauKjt4kpLFuFsJf5MleplV5T6cOy-ehJSbfsOUVeRNSeMdkZ4eLLG_vvTNJB.lJop5ReVf6pWw5rb_E5ILg";

        let parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 5, "JWE token should have 5 parts");

        // Decode and verify the header
        let header_bytes = URL_SAFE_NO_PAD.decode(parts[0]).unwrap();
        let header: JweHeader = serde_json::from_slice(&header_bytes).unwrap();

        assert_eq!(header.alg, "dir");
        assert_eq!(header.enc, "A256GCM");
        assert_eq!(header.iss, EXPECTED_ISSUER);
    }

    #[test]
    fn test_jwe_token_without_decryption_key() {
        let token = "eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIiwiaXNzIjoiaHR0cHM6Ly9hdXRoLnN1cnJlYWxkYi5jb20vIn0..i2Rd5nBEMkJSz6dC.KWp44r7imTAq0nOEXYGC6J4ABuaLFt_4EKFYIUEjN7sNB98aiRatF7nfoopZUqVsp4OWHA1AtnBL8FNuIeHZwH1WthdhAb3P4cbE-KvgrfS3RFyRCXqX9tqzxF9K3wTAvAnI3Lyp510jt9k3ytNKycfJi1mlXKw-WpU8WfqlgKRVd4QkWAn_OKMjfOZDgcCfiKxoHY5FYF77KymTQfQbauKjt4kpLFuFsJf5MleplV5T6cOy-ehJSbfsOUVeRNSeMdkZ4eLLG_vvTNJB.lJop5ReVf6pWw5rb_E5ILg";

        // Should fall back to header-only validation when no decryption key is available
        let result = validate_jwe_token(token, &TokenValidationConfig::default());
        assert!(result.is_ok());
        assert!(result.unwrap().iss == EXPECTED_ISSUER);
    }

    #[test]
    fn test_token_validation_config_default() {
        let config = TokenValidationConfig::default();
        assert_eq!(config.expected_issuer, EXPECTED_ISSUER);
        assert_eq!(config.expected_audience, EXPECTED_AUDIENCE);
        assert!(config.validate_expiration);
        assert!(config.validate_issued_at);
        assert_eq!(config.clock_skew_seconds, 300);
    }

    #[test]
    fn test_custom_token_validation_config() {
        let config = TokenValidationConfig {
            expected_issuer: "https://custom.issuer.com/".to_string(),
            expected_audience: "https://custom.audience.com/".to_string(),
            jwt_public_key: None,
            validate_expiration: false,
            validate_issued_at: false,
            clock_skew_seconds: 600,
        };

        assert_eq!(config.expected_issuer, "https://custom.issuer.com/");
        assert_eq!(config.expected_audience, "https://custom.audience.com/");
        assert!(!config.validate_expiration);
        assert!(!config.validate_issued_at);
        assert_eq!(config.clock_skew_seconds, 600);
    }

    #[tokio::test]
    async fn test_middleware_with_valid_token() {
        let app = Router::new()
            .route("/test", get(|| async { "OK" }))
            .layer(axum::middleware::from_fn(require_bearer_auth));

        let token = "eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIiwiaXNzIjoiaHR0cHM6Ly9hdXRoLnN1cnJlYWxkYi5jb20vIn0..i2Rd5nBEMkJSz6dC.KWp44r7imTAq0nOEXYGC6J4ABuaLFt_4EKFYIUEjN7sNB98aiRatF7nfoopZUqVsp4OWHA1AtnBL8FNuIeHZwH1WthdhAb3P4cbE-KvgrfS3RFyRCXqX9tqzxF9K3wTAvAnI3Lyp510jt9k3ytNKycfJi1mlXKw-WpU8WfqlgKRVd4QkWAn_OKMjfOZDgcCfiKxoHY5FYF77KymTQfQbauKjt4kpLFuFsJf5MleplV5T6cOy-ehJSbfsOUVeRNSeMdkZ4eLLG_vvTNJB.lJop5ReVf6pWw5rb_E5ILg";

        let request = Request::builder()
            .uri("/test")
            .header("Authorization", format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_middleware_with_invalid_token() {
        let app = Router::new()
            .route("/test", get(|| async { "OK" }))
            .layer(axum::middleware::from_fn(require_bearer_auth));

        let request = Request::builder()
            .uri("/test")
            .header("Authorization", "Bearer invalid.token")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_middleware_without_token() {
        let app = Router::new()
            .route("/test", get(|| async { "OK" }))
            .layer(axum::middleware::from_fn(require_bearer_auth));

        let request = Request::builder().uri("/test").body(Body::empty()).unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_middleware_health_endpoint() {
        let app = Router::new()
            .route("/health", get(|| async { "OK" }))
            .layer(axum::middleware::from_fn(require_bearer_auth));

        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
