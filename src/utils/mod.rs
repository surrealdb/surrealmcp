/// Generate a unique connection ID
pub fn generate_connection_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let random = rand::random::<u32>();
    format!("conn_{timestamp:x}_{random:x}")
}

/// Format duration in a human-readable way
pub fn format_duration(duration: std::time::Duration) -> String {
    let total_secs = duration.as_secs();
    let millis = duration.subsec_millis();

    if total_secs == 0 {
        format!("{millis}ms")
    } else if total_secs < 60 {
        format!("{total_secs}.{millis:03}s")
    } else if total_secs < 3600 {
        let minutes = total_secs / 60;
        let seconds = total_secs % 60;
        format!("{minutes}m {seconds}s")
    } else {
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;
        format!("{hours}h {minutes}m {seconds}s")
    }
}

/// Convert various types to SurrealDB Value
///
/// This function safely converts serde_json::Value or String to a SurrealDB Value,
/// providing detailed error messages for conversion failures.
///
/// # Arguments
/// * `value` - The value to convert (serde_json::Value or String)
/// * `name` - The name of the parameter being converted (for error messages)
///
/// # Returns
/// * `Ok(Value)` - The converted SurrealDB Value
/// * `Err(String)` - Error message if conversion fails
///
/// # Examples
/// ```
/// use surrealmcp::utils;
///
/// // Convert JSON value
/// let json_val = serde_json::json!({"name": "John"});
/// let surreal_val = utils::convert_to_surreal_value(json_val, "user_data")?;
///
/// // Convert string directly
/// let string_val = "table_name".to_string();
/// let surreal_val = utils::convert_to_surreal_value(string_val, "table")?;
/// ```
pub fn convert_to_surreal_value(
    value: impl Into<serde_json::Value>,
    name: &str,
) -> Result<surrealdb::Value, String> {
    let json_value = value.into();
    serde_json::from_value::<surrealdb::Value>(json_value)
        .map_err(|e| format!("Failed to convert parameter {name}: {e}"))
}
