use axum::http::Request;
use axum::middleware::Next;
use axum::{
    http::StatusCode,
    http::header::{AUTHORIZATION, WWW_AUTHENTICATE},
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use josekit::jwe::JweContext;
use josekit::jwe::alg::direct::DirectJweAlgorithm;
use josekit::jwk::Jwk;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// WWW-Authenticate value for HTTP 401 responses
const WWW_AUTHENTICATE_VALUE: &str =
    "Bearer resource_metadata='/.well-known/oauth-protected-resource'";

/// Expected issuer for SurrealDB auth tokens
const EXPECTED_ISSUER: &str = "https://auth.surrealdb.com/";

/// Expected audience for SurrealDB MCP tokens
const EXPECTED_AUDIENCE: &str = "https://mcp.surrealdb.com/";

/// JWKS endpoint for SurrealDB auth
const JWKS_ENDPOINT: &str = "https://auth.surrealdb.com/.well-known/jwks.json";

/// JWKS cache duration (1 hour)
const JWKS_CACHE_DURATION: Duration = Duration::from_secs(3600);

/// JWKS (JSON Web Key Set) structure
#[derive(Debug, Serialize, Deserialize)]
struct Jwks {
    keys: Vec<JwksKey>,
}

/// JWK (JSON Web Key) structure
#[derive(Debug, Serialize, Deserialize, Clone)]
struct JwksKey {
    #[serde(rename = "kty")]
    key_type: String,
    #[serde(rename = "kid")]
    key_id: String,
    #[serde(rename = "use")]
    key_use: Option<String>,
    #[serde(rename = "alg")]
    algorithm: Option<String>,
    #[serde(rename = "n")]
    modulus: Option<String>,
    #[serde(rename = "e")]
    exponent: Option<String>,
    #[serde(rename = "x")]
    x_coordinate: Option<String>,
    #[serde(rename = "y")]
    y_coordinate: Option<String>,
    #[serde(rename = "crv")]
    curve: Option<String>,
}

/// Cached JWKS with expiration
#[derive(Debug, Clone)]
struct CachedJwks {
    keys: HashMap<String, JwksKey>,
    expires_at: SystemTime,
}

impl CachedJwks {
    /// Create a new JWKS cache
    fn new(keys: Vec<JwksKey>) -> Self {
        // Create a new hash map to store the JWKS
        let mut store = HashMap::new();
        // Insert the JWKS into the hash map
        for key in keys {
            store.insert(key.key_id.clone(), key);
        }
        // Create a new cached JWKS
        Self {
            keys: store,
            expires_at: SystemTime::now() + JWKS_CACHE_DURATION,
        }
    }

    /// Checks if the JWKS cache is expired
    fn is_expired(&self) -> bool {
        SystemTime::now() > self.expires_at
    }

    /// Gets a JWK by key ID
    fn get_key(&self, kid: &str) -> Option<&JwksKey> {
        self.keys.get(kid)
    }
}

/// JWKS manager for fetching and caching public keys
#[derive(Debug, Clone)]
pub struct JwksManager {
    /// HTTP client for fetching JWKS
    client: reqwest::Client,
    /// Temporary cache for JWKS
    cache: Arc<RwLock<Option<CachedJwks>>>,
}

impl JwksManager {
    /// Create a new JWKS manager
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Fetch JWKS from the authentication endpoint
    async fn fetch_jwks(&self) -> Result<Jwks, String> {
        // Output debugging information
        debug!("Fetching JWKS from {JWKS_ENDPOINT}");
        // Fetch the JWKS from the endpoint
        let response = self
            .client
            .get(JWKS_ENDPOINT)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch JWKS: {e}"))?;
        // Check if the response is successful
        if !response.status().is_success() {
            return Err(format!(
                "JWK endpoint returned error status: {}",
                response.status()
            ));
        }
        // Parse the response as JSON
        let jwks: Jwks = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse JWKS JSON: {e}"))?;
        // Output debugging information
        info!("Successfully fetched JWKS with {} keys", jwks.keys.len());
        // Return the JWKS
        Ok(jwks)
    }

    /// Gets cached JWKS or fetches JWKS if expired
    async fn get_jwks(&self) -> Result<CachedJwks, String> {
        // Acquire a write lock on the cache
        let mut cache = self.cache.write().await;
        // Check if we have a valid cached JWKS
        if let Some(cache) = cache.as_ref()
            && !cache.is_expired()
        {
            return Ok(cache.clone());
        }
        // Fetch new JWKS
        debug!("JWK cache expired or missing, fetching new JWKS");
        // Fetch the updated JWKS
        let jwks = self.fetch_jwks().await?;
        // Create a new JWKS cache
        let cached_jwks = CachedJwks::new(jwks.keys);
        // Update the temporary cache
        *cache = Some(cached_jwks.clone());
        // Output debugging information
        debug!("Successfully updated JWK cache");
        // Return the cached JWKS
        Ok(cached_jwks)
    }

    /// Get a decoding key for a specific key ID
    pub async fn get_decoding_key(&self, kid: &str) -> Result<DecodingKey, String> {
        // Get the cached JWKS
        let cached_jwks = self.get_jwks().await?;
        // Get the specific JWK
        let jwk = cached_jwks
            .get_key(kid)
            .ok_or_else(|| format!("Key ID '{kid}' not found in JWKS"))?;

        match jwk.key_type.as_str() {
            "RSA" => {
                let n = jwk
                    .modulus
                    .as_ref()
                    .ok_or_else(|| "RSA key missing modulus".to_string())?;
                let e = jwk
                    .exponent
                    .as_ref()
                    .ok_or_else(|| "RSA key missing exponent".to_string())?;
                DecodingKey::from_rsa_components(n, e)
                    .map_err(|e| format!("Failed to create RSA decoding key: {e}"))
            }
            "EC" => {
                let x = jwk
                    .x_coordinate
                    .as_ref()
                    .ok_or_else(|| "EC key missing x coordinate".to_string())?;
                let y = jwk
                    .y_coordinate
                    .as_ref()
                    .ok_or_else(|| "EC key missing y coordinate".to_string())?;
                DecodingKey::from_ec_components(x, y)
                    .map_err(|e| format!("Failed to create EC decoding key: {e}"))
            }
            v => Err(format!("Unsupported key type: {v}")),
        }
    }
}

/// Token validation configuration
#[derive(Clone)]
pub struct TokenValidationConfig {
    /// Expected issuer for authentication tokens
    pub expected_issuer: String,
    /// Expected audience for authentication tokens
    pub expected_audience: String,
    /// Public key for JWT validation
    pub jwt_public_key: Option<String>,
    /// Base64-encoded key for JWE decryption
    pub jwe_decryption_key: Option<String>,
    /// Whether to validate token expiration
    pub validate_expiration: bool,
    /// Whether to validate token issued at
    pub validate_issued_at: bool,
    /// Clock skew in seconds
    pub clock_skew_seconds: u64,
    /// JWKS manager for fetching and caching public keys
    pub jwks_manager: Option<JwksManager>,
}

impl Default for TokenValidationConfig {
    fn default() -> Self {
        Self {
            expected_issuer: EXPECTED_ISSUER.to_string(),
            expected_audience: EXPECTED_AUDIENCE.to_string(),
            jwt_public_key: None,
            jwe_decryption_key: None,
            validate_expiration: true,
            validate_issued_at: true,
            clock_skew_seconds: 300, // 5 minutes
            jwks_manager: Some(JwksManager::new()),
        }
    }
}

/// JWE header structure
#[derive(Debug, Serialize, Deserialize)]
struct JweHeader {
    /// The algorithm used to encrypt the token
    alg: String,
    /// The encryption algorithm used to encrypt the token
    enc: String,
    /// The issuer of the token
    iss: String,
}

/// JWT header structure
#[derive(Debug, Serialize, Deserialize)]
struct JwtHeader {
    /// The algorithm used to sign the token
    alg: String,
    /// The key ID of the token
    kid: Option<String>,
    /// The type of the token
    typ: Option<String>,
}

/// Token claims structure for both JWE and JWT tokens
#[derive(Debug, Serialize, Deserialize)]
struct TokenClaims {
    /// The issuer of the token
    iss: String,
    /// The audience of the token
    aud: Option<String>,
    /// The expiration time of the token
    exp: Option<u64>,
    /// The issued at time of the token
    iat: Option<u64>,
    /// The subject of the token
    sub: Option<String>,
}

/// Validate and decrypt a JWE token from SurrealDB auth service
///
/// This function validates the JWE token header structure and issuer,
/// and if a decryption key is provided, decrypts the token to access full claims.
/// For SurrealDB tokens using "dir" algorithm with A256GCM encryption.
async fn validate_jwe_token(
    token: &str,
    config: &TokenValidationConfig,
) -> Result<TokenClaims, String> {
    // Output debugging information
    debug!(token = %token, "Validating JWE token");
    // JWE tokens have 5 parts separated by dots
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 5 {
        return Err("Invalid JWE token format: expected 5 parts".to_string());
    }
    // Decode the header into bytes
    let header_bytes = URL_SAFE_NO_PAD
        .decode(parts[0])
        .map_err(|e| format!("Failed to decode JWE header: {e}"))?;
    // Parse the header contents
    let header: JweHeader = serde_json::from_slice(&header_bytes)
        .map_err(|e| format!("Failed to parse JWE header: {e}"))?;
    // Validate the algorithm
    if header.alg != "dir" {
        return Err(format!(
            "Unsupported key management algorithm: {}",
            header.alg
        ));
    }
    // Validate the encryption
    if header.enc != "A256GCM" {
        return Err(format!(
            "Unsupported content encryption algorithm: {}",
            header.enc
        ));
    }
    // Check if we have a decryption key
    if let Some(decryption_key) = &config.jwe_decryption_key {
        // Perform full token validation when decryption key is available
        debug!("JWE decryption key provided, performing token validation");
        // Decode the decryption key from base64
        let key_bytes = URL_SAFE_NO_PAD
            .decode(decryption_key)
            .map_err(|e| format!("Failed to decode decryption key: {e}"))?;
        // Create a JWK for decryption
        let mut jwk = Jwk::new("oct");
        // Set the JWK decryption key
        jwk.set_parameter(
            "k",
            Some(serde_json::Value::String(
                base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&key_bytes),
            )),
        )
        .map_err(|e| format!("Failed to set JWK parameter: {e}"))?;
        // Create a JWE algorithm for direct key algorithm
        let algorithm = DirectJweAlgorithm::Dir;
        // Create a JWE decrypter for direct key algorithm
        let decrypter = algorithm
            .decrypter_from_jwk(&jwk)
            .map_err(|e| format!("Failed to create JWE decrypter: {e}"))?;
        // Create JWE context for deserialization
        let jwe_context = JweContext::new();
        // Deserialize and decrypt the JWE token
        let (decrypted, _header) = jwe_context
            .deserialize_compact(token.as_bytes(), &decrypter)
            .map_err(|e| format!("Failed to decrypt JWE token: {e}"))?;
        // Parse the decrypted payload as JWT claims
        let payload_str = String::from_utf8(decrypted)
            .map_err(|e| format!("Failed to convert decrypted payload to string: {e}"))?;
        // Parse the decrypted payload as JWT claims
        let claims: TokenClaims = serde_json::from_str(&payload_str)
            .map_err(|e| format!("Failed to parse decrypted JWT claims: {e}"))?;
        // Validate the issuer from decrypted claims
        if claims.iss != config.expected_issuer {
            return Err(format!(
                "Invalid issuer: expected {}, got {}",
                config.expected_issuer, claims.iss
            ));
        }
        // Validate expiration if enabled
        if config.validate_expiration
            && let Some(exp) = claims.exp
        {
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| format!("Failed to get current time: {e}"))?
                .as_secs();
            if current_time > exp + config.clock_skew_seconds {
                return Err(format!(
                    "Token 'exp' invalid: expired at {exp}, current time {current_time}",
                ));
            }
        }
        // Validate issued at if enabled
        if config.validate_issued_at
            && let Some(iat) = claims.iat
        {
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| format!("Failed to get current time: {e}"))?
                .as_secs();
            if iat > current_time + config.clock_skew_seconds {
                return Err(format!(
                    "Token 'iat' invalid: issued at {iat}, current time {current_time}",
                ));
            }
        }
        // Output debugging information
        debug!(
            token = %token,
            issuer = %claims.iss,
            audience = ?claims.aud,
            subject = ?claims.sub,
            expiration = ?claims.exp,
            issued_at = ?claims.iat,
            "JWE token validated successfully (with decryption key)"
        );
        // Return the claims
        Ok(claims)
    } else {
        // Fallback to header-only validation when no decryption key is available
        debug!("No JWE decryption key provided, performing header validation");
        // Validate the issuer from header
        if header.iss != config.expected_issuer {
            return Err(format!(
                "Invalid issuer: expected {}, got {}",
                config.expected_issuer, header.iss
            ));
        }
        // Create the default claims
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
            "JWE token header validated successfully (without decryption key)"
        );
        // Return the claims
        Ok(claims)
    }
}

/// Validate a standard JWT token using JWKS
///
/// This function validates JWT tokens using the jsonwebtoken crate and JWKS.
/// It validates all claims including audience, expiration, issued at, and subject.
async fn validate_jwt_token(
    token: &str,
    config: &TokenValidationConfig,
) -> Result<TokenClaims, String> {
    // Output debugging information
    debug!(token = %token, "Validating JWT token");
    // Decode the header to check the algorithm and key ID
    let header = decode_header(token).map_err(|e| format!("Failed to decode JWT header: {e}"))?;
    // Create validation configuration
    let mut validation = Validation::new(header.alg);
    validation.set_audience(&[&config.expected_audience]);
    validation.set_issuer(&[&config.expected_issuer]);
    validation.set_required_spec_claims(&["iss", "aud", "exp", "iat", "sub"]);
    validation.leeway = config.clock_skew_seconds;
    validation.validate_aud = true;
    validation.validate_exp = true;
    // Get the decoding key
    let key = if let Some(jwks_manager) = &config.jwks_manager {
        // Get the key ID from the header
        let kid = header
            .kid
            .ok_or_else(|| "JWT token missing key ID (kid)".to_string())?;
        // Output debugging information
        debug!(kid = %kid, "JWT token has key ID");
        // Use JWKS to get the public key
        jwks_manager
            .get_decoding_key(&kid)
            .await
            .map_err(|e| format!("Failed to get decoding key from JWKS: {e}"))?
    } else if let Some(public_key) = &config.jwt_public_key {
        // Fallback to static public key
        match header.alg {
            Algorithm::RS256 | Algorithm::RS384 | Algorithm::RS512 => {
                // Decode the RSA public key
                DecodingKey::from_rsa_pem(public_key.as_bytes())
                    .map_err(|e| format!("Failed to create RSA decoding key: {e}"))?
            }
            Algorithm::ES256 | Algorithm::ES384 => {
                // Decode the EC public key
                DecodingKey::from_ec_pem(public_key.as_bytes())
                    .map_err(|e| format!("Failed to create EC decoding key: {e}"))?
            }
            v => {
                return Err(format!("Unsupported JWT algorithm: {v:?}"));
            }
        }
    } else {
        // Fallback to dummy key for testing
        DecodingKey::from_secret(b"dummy-key")
    };
    // Decode the authentication token
    let token_data = decode::<TokenClaims>(token, &key, &validation)
        .map_err(|e| format!("Failed to validate JWT token: {e}"))?;
    // Validate expiration time
    if config.validate_expiration
        && let Some(exp) = token_data.claims.exp
    {
        // Get the current time
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("Failed to get current time: {e}"))?
            .as_secs();
        // Check for expiration, allowing for clock skew
        if current_time > exp + config.clock_skew_seconds {
            return Err(format!(
                "Token 'exp' invalid: expired at {exp}, current time {current_time}",
            ));
        }
    }
    // Validate issued at time
    if config.validate_issued_at
        && let Some(iat) = token_data.claims.iat
    {
        // Get the current time
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("Failed to get current time: {e}"))?
            .as_secs();
        // Check for issued at, allowing for clock skew
        if iat > current_time + config.clock_skew_seconds {
            return Err(format!(
                "Token 'iat' invalid: issued at {iat}, current time {current_time}",
            ));
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
async fn validate_bearer_token(
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
        5 => validate_jwe_token(token, config).await,
        3 => validate_jwt_token(token, config).await,
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
/// 4. Stores the validated token in the context extensions for use by subsequent services
/// 5. Returns 401 Unauthorized with proper WWW-Authenticate header if validation fails
///
/// Security considerations:
/// - Validates token structure and issuer
/// - For JWT tokens: validates audience, expiration, and issued at using JWKS
/// - For JWE tokens: validates header structure only (requires decryption key for full validation)
/// - Supports both JWE and JWT token formats
/// - Logs validation failures for monitoring
/// - Uses proper HTTP status codes and headers
pub async fn require_bearer_auth(
    config: TokenValidationConfig,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Get the current request path
    let path = req.uri().path();
    // Allow access to auth metadata and health check endpoint
    if path.starts_with("/.well-known/") || path == "/health" {
        return Ok(next.run(req).await);
    }
    // Extract the bearer token from the Authorization header
    let bearer_token = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(|s| s.to_string());
    // If the header is present, validate the token
    if let Some(token) = bearer_token {
        match validate_bearer_token(&token, &config).await {
            Ok(claims) => {
                debug!(
                    issuer = %claims.iss,
                    audience = ?claims.aud,
                    subject = claims.sub.as_deref().unwrap_or("unknown"),
                    expiration = ?claims.exp,
                    issued_at = ?claims.iat,
                    "Bearer token validated successfully"
                );
                // Store the token on the request context
                req.extensions_mut().insert(token);
                // Continue to the next middleware
                return Ok(next.run(req).await);
            }
            Err(e) => {
                warn!("Bearer token validation failed: {e}");
                // Continue to return 401
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

    #[tokio::test]
    async fn test_validate_surrealdb_jwe_token() {
        // Example JWE token from SurrealDB auth service
        let token = "eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIiwiaXNzIjoiaHR0cHM6Ly9hdXRoLnN1cnJlYWxkYi5jb20vIn0..i2Rd5nBEMkJSz6dC.KWp44r7imTAq0nOEXYGC6J4ABuaLFt_4EKFYIUEjN7sNB98aiRatF7nfoopZUqVsp4OWHA1AtnBL8FNuIeHZwH1WthdhAb3P4cbE-KvgrfS3RFyRCXqX9tqzxF9K3wTAvAnI3Lyp510jt9k3ytNKycfJi1mlXKw-WpU8WfqlgKRVd4QkWAn_OKMjfOZDgcCfiKxoHY5FYF77KymTQfQbauKjt4kpLFuFsJf5MleplV5T6cOy-ehJSbfsOUVeRNSeMdkZ4eLLG_vvTNJB.lJop5ReVf6pWw5rb_E5ILg";

        let result = validate_jwe_token(token, &TokenValidationConfig::default()).await;
        assert!(
            result.is_ok(),
            "Token validation should succeed: {result:?}"
        );

        let claims = result.unwrap();
        assert_eq!(claims.iss, EXPECTED_ISSUER);
    }

    #[tokio::test]
    async fn test_validate_empty_token() {
        let result = validate_jwe_token("", &TokenValidationConfig::default()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid JWE token format"));
    }

    #[tokio::test]
    async fn test_validate_invalid_format() {
        let result = validate_jwe_token("invalid.token", &TokenValidationConfig::default()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid JWE token format"));
    }

    #[tokio::test]
    async fn test_validate_jwe_header_structure() {
        let token = "eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIiwiaXNzIjoiaHR0cHM6Ly9hdXRoLnN1cnJlYWxkYi5jb20vIn0..i2Rd5nBEMkJSz6dC.KWp44r7imTAq0nOEXYGC6J4ABuaLFt_4EKFYIUEjN7sNB98aiRatF7nfoopZUqVsp4OWHA1AtnBL8FNuIeHZwH1WthdhAb3P4cbE-KvgrfS3RFyRCXqX9tqzxF9K3wTAvAnI3Lyp510jt9k3ytNKycfJi1mlXKw-WpU8WfqlgKRVd4QkWAn_OKMjfOZDgcCfiKxoHY5FYF77KymTQfQbauKjt4kpLFuFsJf5MleplV5T6cOy-ehJSbfsOUVeRNSeMdkZ4eLLG_vvTNJB.lJop5ReVf6pWw5rb_E5ILg";

        let result = validate_jwe_token(token, &TokenValidationConfig::default()).await;
        assert!(result.is_ok(), "JWE token validation should succeed");

        let claims = result.unwrap();
        assert_eq!(claims.iss, EXPECTED_ISSUER);
    }

    #[tokio::test]
    async fn test_jwe_token_without_decryption_key() {
        let token = "eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIiwiaXNzIjoiaHR0cHM6Ly9hdXRoLnN1cnJlYWxkYi5jb20vIn0..i2Rd5nBEMkJSz6dC.KWp44r7imTAq0nOEXYGC6J4ABuaLFt_4EKFYIUEjN7sNB98aiRatF7nfoopZUqVsp4OWHA1AtnBL8FNuIeHZwH1WthdhAb3P4cbE-KvgrfS3RFyRCXqX9tqzxF9K3wTAvAnI3Lyp510jt9k3ytNKycfJi1mlXKw-WpU8WfqlgKRVd4QkWAn_OKMjfOZDgcCfiKxoHY5FYF77KymTQfQbauKjt4kpLFuFsJf5MleplV5T6cOy-ehJSbfsOUVeRNSeMdkZ4eLLG_vvTNJB.lJop5ReVf6pWw5rb_E5ILg";

        // Should fall back to header-only validation when no decryption key is available
        let result = validate_jwe_token(token, &TokenValidationConfig::default()).await;
        assert!(result.is_ok());
        assert!(result.unwrap().iss == EXPECTED_ISSUER);
    }

    #[tokio::test]
    async fn test_jwe_token_with_decryption_key() {
        // This test demonstrates how to configure JWE decryption
        // In a real scenario, you would provide the actual decryption key
        let config = TokenValidationConfig {
            jwe_decryption_key: Some("base64-encoded-32-byte-key".to_string()),
            ..Default::default()
        };

        // This would be a real JWE token encrypted with the provided key
        let token = "eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIiwiaXNzIjoiaHR0cHM6Ly9hdXRoLnN1cnJlYWxkYi5jb20vIn0..i2Rd5nBEMkJSz6dC.KWp44r7imTAq0nOEXYGC6J4ABuaLFt_4EKFYIUEjN7sNB98aiRatF7nfoopZUqVsp4OWHA1AtnBL8FNuIeHZwH1WthdhAb3P4cbE-KvgrfS3RFyRCXqX9tqzxF9K3wTAvAnI3Lyp510jt9k3ytNKycfJi1mlXKw-WpU8WfqlgKRVd4QkWAn_OKMjfOZDgcCfiKxoHY5FYF77KymTQfQbauKjt4kpLFuFsJf5MleplV5T6cOy-ehJSbfsOUVeRNSeMdkZ4eLLG_vvTNJB.lJop5ReVf6pWw5rb_E5ILg";

        // The test should fail because the decryption key is invalid
        // This demonstrates that the JWE decryption is being attempted
        let result = validate_jwe_token(token, &config).await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(
            error.contains("Failed to decode decryption key")
                || error.contains("Failed to decrypt JWE token")
        );
    }

    #[test]
    fn test_token_validation_config_default() {
        let config = TokenValidationConfig::default();
        assert_eq!(config.expected_issuer, EXPECTED_ISSUER);
        assert_eq!(config.expected_audience, EXPECTED_AUDIENCE);
        assert!(config.validate_expiration);
        assert!(config.validate_issued_at);
        assert_eq!(config.clock_skew_seconds, 300);
        assert!(config.jwks_manager.is_some());
    }

    #[test]
    fn test_custom_token_validation_config() {
        let config = TokenValidationConfig {
            expected_issuer: "https://custom.issuer.com/".to_string(),
            expected_audience: "https://custom.audience.com/".to_string(),
            jwt_public_key: None,
            jwe_decryption_key: Some("custom-jwe-key".to_string()),
            validate_expiration: false,
            validate_issued_at: false,
            clock_skew_seconds: 600,
            jwks_manager: None,
        };

        assert_eq!(config.expected_issuer, "https://custom.issuer.com/");
        assert_eq!(config.expected_audience, "https://custom.audience.com/");
        assert_eq!(
            config.jwe_decryption_key,
            Some("custom-jwe-key".to_string())
        );
        assert!(!config.validate_expiration);
        assert!(!config.validate_issued_at);
        assert_eq!(config.clock_skew_seconds, 600);
        assert!(config.jwks_manager.is_none());
    }

    #[tokio::test]
    async fn test_middleware_with_valid_token() {
        let app =
            Router::new()
                .route("/test", get(|| async { "OK" }))
                .layer(axum::middleware::from_fn(|req, next| {
                    let config = TokenValidationConfig::default();
                    require_bearer_auth(config, req, next)
                }));

        let token = "eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIiwiaXNzIjoiaHR0cHM6Ly9hdXRoLnN1cnJlYWxkYi5jb20vIn0..i2Rd5nBEMkJSz6dC.KWp44r7imTAq0nOEXYGC6J4ABuaLFt_4EKFYIUEjN7sNB98aiRatF7nfoopZUqVsp4OWHA1AtnBL8FNuIeHZwH1WthdhAb3P4cbE-KvgrfS3RFyRCXqX9tqzxF9K3wTAvAnI3Lyp510jt9k3ytNKycfJi1mlXKw-WpU8WfqlgKRVd4QkWAn_OKMjfOZDgcCfiKxoHY5FYF77KymTQfQbauKjt4kpLFuFsJf5MleplV5T6cOy-ehJSbfsOUVeRNSeMdkZ4eLLG_vvTNJB.lJop5ReVf6pWw5rb_E5ILg";

        let request = Request::builder()
            .uri("/test")
            .header("Authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_middleware_with_invalid_token() {
        let app =
            Router::new()
                .route("/test", get(|| async { "OK" }))
                .layer(axum::middleware::from_fn(|req, next| {
                    let config = TokenValidationConfig::default();
                    require_bearer_auth(config, req, next)
                }));

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
        let app =
            Router::new()
                .route("/test", get(|| async { "OK" }))
                .layer(axum::middleware::from_fn(|req, next| {
                    let config = TokenValidationConfig::default();
                    require_bearer_auth(config, req, next)
                }));

        let request = Request::builder().uri("/test").body(Body::empty()).unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_middleware_health_endpoint() {
        let app = Router::new()
            .route("/health", get(|| async { "OK" }))
            .layer(axum::middleware::from_fn(|req, next| {
                let config = TokenValidationConfig::default();
                require_bearer_auth(config, req, next)
            }));

        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_jwks_manager_creation() {
        let manager = JwksManager::new();
        assert!(manager.cache.read().await.is_none());
    }

    #[tokio::test]
    async fn test_cached_jwks_expiration() {
        let jwks = Jwks {
            keys: vec![JwksKey {
                key_type: "RSA".to_string(),
                key_id: "test-key".to_string(),
                key_use: Some("sig".to_string()),
                algorithm: Some("RS256".to_string()),
                modulus: Some("test-modulus".to_string()),
                exponent: Some("test-exponent".to_string()),
                x_coordinate: None,
                y_coordinate: None,
                curve: None,
            }],
        };

        let cached_jwks = CachedJwks::new(jwks.keys);
        assert!(!cached_jwks.is_expired());

        // Test that we can get the key
        let key = cached_jwks.get_key("test-key");
        assert!(key.is_some());
        assert_eq!(key.unwrap().key_id, "test-key");

        // Test that non-existent key returns None
        let key = cached_jwks.get_key("non-existent");
        assert!(key.is_none());
    }

    #[tokio::test]
    async fn test_jwks_fetching() {
        let manager = JwksManager::new();

        // Test fetching JWKS from SurrealDB auth endpoint
        let result = manager.fetch_jwks().await;

        // The test should either succeed (if the endpoint is available) or fail gracefully
        match result {
            Ok(jwks) => {
                info!("Successfully fetched JWKS with {} keys", jwks.keys.len());
                assert!(
                    !jwks.keys.is_empty(),
                    "JWKS should contain at least one key"
                );

                // Test that we can get a decoding key for the first key
                if let Some(first_key) = jwks.keys.first() {
                    let key_result = manager.get_decoding_key(&first_key.key_id).await;
                    // This might fail due to key construction limitations, but shouldn't panic
                    match key_result {
                        Ok(_) => info!(
                            "Successfully created decoding key for key ID: {}",
                            first_key.key_id
                        ),
                        Err(e) => warn!("Failed to create decoding key: {}", e),
                    }
                }
            }
            Err(e) => {
                warn!(
                    "Failed to fetch JWKS (this is expected in test environment): {}",
                    e
                );
                // This is expected in test environments where the endpoint might not be available
            }
        }
    }

    #[tokio::test]
    async fn test_custom_audience_configuration() {
        let custom_config = TokenValidationConfig {
            expected_audience: "https://custom.audience.com/".to_string(),
            ..Default::default()
        };

        assert_eq!(
            custom_config.expected_audience,
            "https://custom.audience.com/"
        );
        assert_eq!(custom_config.expected_issuer, EXPECTED_ISSUER);
        assert!(custom_config.validate_expiration);
        assert!(custom_config.validate_issued_at);
    }

    #[tokio::test]
    async fn test_jwe_decryption_key_configuration() {
        let custom_config = TokenValidationConfig {
            jwe_decryption_key: Some("base64-encoded-32-byte-key".to_string()),
            ..Default::default()
        };

        assert_eq!(
            custom_config.jwe_decryption_key,
            Some("base64-encoded-32-byte-key".to_string())
        );
        assert_eq!(custom_config.expected_issuer, EXPECTED_ISSUER);
        assert_eq!(custom_config.expected_audience, EXPECTED_AUDIENCE);
        assert!(custom_config.validate_expiration);
        assert!(custom_config.validate_issued_at);
    }
}
