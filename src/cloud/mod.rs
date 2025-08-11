use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, error, info, trace};

const CLOUD_API_BASE_URL: &str = "https://api.cloud.surrealdb.com/api/v1";

/// A response from signing in to SurrealDB Cloud
#[derive(Debug, Serialize, Deserialize)]
pub struct CloudSignInResponse {
    pub id: String,
    pub token: String,
}

/// A user in SurrealDB Cloud
#[derive(Debug, Serialize, Deserialize)]
pub struct CloudUser {
    pub id: String,
    pub email: String,
    pub name: String,
}

/// An organization in SurrealDB Cloud
#[derive(Debug, Serialize, Deserialize)]
pub struct CloudOrganization {
    pub id: String,
    pub name: String,
    pub user_role: Option<String>,
    pub billing_info: Option<bool>,
    pub payment_info: Option<bool>,
    pub max_free_instances: Option<i32>,
    pub max_paid_instances: Option<i32>,
    pub member_count: Option<i32>,
    pub plan: Option<CloudPlan>,
}

/// A plan in SurrealDB Cloud
#[derive(Debug, Serialize, Deserialize)]
pub struct CloudPlan {
    pub id: String,
    pub name: String,
    pub description: String,
    pub regions: Vec<String>,
}

/// A cloud instance in SurrealDB Cloud
#[derive(Debug, Serialize, Deserialize)]
pub struct CloudInstance {
    pub id: String,
    pub name: String,
    pub slug: Option<String>,
    pub version: Option<String>,
    pub available_versions: Option<Vec<String>>,
    pub host: Option<String>,
    pub region: Option<String>,
    pub organization_id: Option<String>,
    pub compute_units: Option<i32>,
    pub state: Option<String>,
    pub storage_size: Option<i32>,
    pub can_update_storage_size: Option<bool>,
    pub storage_size_update_cooloff_hours: Option<i32>,
}

/// A response from getting auth token for a cloud instance
#[derive(Debug, Serialize, Deserialize)]
pub struct CloudInstanceAuth {
    pub token: String,
}

/// A cloud instance status in SurrealDB Cloud
#[derive(Debug, Serialize, Deserialize)]
pub struct CloudInstanceStatus {
    pub phase: String,
    pub db_backups: Vec<CloudInstanceBackup>,
}

/// A cloud instance backup in SurrealDB Cloud
#[derive(Debug, Serialize, Deserialize)]
pub struct CloudInstanceBackup {
    pub snapshot_started_at: String,
    pub snapshot_id: String,
}

/// A request to create a cloud instance
#[derive(Debug, Serialize, Deserialize)]
pub struct CloudCreateInstanceRequest {
    pub name: String,
    pub organization_id: String,
}

/// A response from creating a cloud instance
#[derive(Debug, Serialize, Deserialize)]
pub struct CloudCreateInstanceResponse {
    pub instance: CloudInstance,
}

/// A client for SurrealDB Cloud
pub struct Client {
    /// The HTTP client
    client: reqwest::Client,
    /// The MCP client token
    pub client_token: RwLock<Option<String>>,
    /// The SurrealDB Cloud auth token
    pub auth_token: RwLock<Option<String>>,
    /// The SurrealDB Cloud refresh token
    pub refresh_token: RwLock<Option<String>>,
}

impl Client {
    /// Create a new SurrealDB Cloud client
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            client_token: RwLock::new(None),
            auth_token: RwLock::new(None),
            refresh_token: RwLock::new(None),
        }
    }

    /// Create a new SurrealDB Cloud client with pre-configured tokens
    pub fn with_tokens(access_token: String, refresh_token: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            client_token: RwLock::new(None),
            auth_token: RwLock::new(Some(access_token)),
            refresh_token: RwLock::new(Some(refresh_token)),
        }
    }

    /// Send a GET request to the given URL
    async fn get(&self, url: &str) -> Result<reqwest::Response> {
        // Ensure we are authenticated
        self.authenticate().await?;
        // Create the full URL path
        let url = format!("{CLOUD_API_BASE_URL}{url}");
        // Await the stored auth token
        let auth_token = self.auth_token.read().await;
        // Get the authentication token
        let auth_token = auth_token
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not authenticated with SurrealDB Cloud"))?;
        // Create the request
        let request = self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {auth_token}"));
        // Output debugging information
        trace!(
            request = ?request,
            "Sending GET request to SurrealDB Cloud",
        );
        // Send the request
        let response = request.send().await?;
        // Return the response
        Ok(response)
    }

    /// Send a POST request to the given URL with the given body
    async fn post<T>(&self, url: &str, body: &T) -> Result<reqwest::Response>
    where
        T: Serialize + ?Sized,
    {
        // Ensure we are authenticated
        self.authenticate().await?;
        // Create the full URL path
        let url = format!("{CLOUD_API_BASE_URL}{url}");
        // Await the stored auth token
        let auth_token = self.auth_token.read().await;
        // Get the authentication token
        let auth_token = auth_token
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not authenticated with SurrealDB Cloud"))?;
        // Create the request
        let request = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {auth_token}"))
            .json(body);
        // Output debugging information
        trace!(
            request = ?request,
            "Sending POST request to SurrealDB Cloud",
        );
        // Send the request
        let response = request.send().await?;
        // Return the response
        Ok(response)
    }

    /// Authenticate with SurrealDB Cloud using a bearer token
    async fn authenticate(&self) -> Result<()> {
        // If the auth token is already set, return
        if self.auth_token.read().await.is_some() {
            return Ok(());
        }
        // Await the stored client token
        let client_token = self.client_token.read().await;
        // Get the client token
        let client_token = client_token
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No authentication token available"))?;
        // Output debugging information
        debug!("Authenticating with SurrealDB Cloud using bearer token");
        // Create the full URL path
        let url = format!("{CLOUD_API_BASE_URL}/signin");
        // Send the request
        let response = self.client.post(url).json(&client_token).send().await?;
        // Check the response status
        if !response.status().is_success() {
            let e = response.text().await?;
            error!("Failed to authenticate with SurrealDB Cloud: {e}");
            return Err(anyhow::anyhow!("Authentication failed: {e}"));
        }
        // Parse the returned response
        let result: CloudSignInResponse = response.json().await?;
        // Store the authentication token
        let mut auth_token = self.auth_token.write().await;
        *auth_token = Some(result.token);
        // Store the refresh token
        let mut refresh_token = self.refresh_token.write().await;
        *refresh_token = Some(result.id);
        // Output debugging information
        info!("Successfully authenticated with SurrealDB Cloud",);
        // Return nothing
        Ok(())
    }

    /// List organizations in SurrealDB Cloud
    pub async fn list_organizations(&self) -> Result<Vec<CloudOrganization>> {
        // Output debugging information
        debug!("Fetching organizations from SurrealDB Cloud");
        // Send the request
        let response = self.get("/organizations").await?;
        // Check the response status
        if !response.status().is_success() {
            let e = response.text().await?;
            error!("Failed to fetch organizations: {e}");
            return Err(anyhow::anyhow!("Failed to fetch organizations: {e}"));
        }
        // Parse the returned response as raw JSON
        let json: serde_json::Value = response.json().await?;
        // Parse the raw JSON into organizations
        let result: Vec<CloudOrganization> = serde_json::from_value(json)?;
        // Output debugging information
        debug!(
            organisations = result.len(),
            "Successfully fetched organizations",
        );
        // Return the organizations
        Ok(result)
    }

    /// List cloud instances in SurrealDB Cloud
    pub async fn list_instances(&self, organization_id: &str) -> Result<Vec<CloudInstance>> {
        // Output debugging information
        debug!(
            organization_id = organization_id,
            "Fetching cloud instances from SurrealDB Cloud",
        );
        // Send the request
        let response = self
            .get(&format!("/organizations/{organization_id}/instances"))
            .await?;
        // Check the response status
        if !response.status().is_success() {
            let e = response.text().await?;
            error!(
                organization_id = organization_id,
                "Failed to fetch cloud instances: {e}",
            );
            return Err(anyhow::anyhow!("Failed to fetch cloud instances: {e}"));
        }
        // Parse the returned response as raw JSON
        let json: serde_json::Value = response.json().await?;
        // Parse the raw JSON into instances
        let result: Vec<CloudInstance> = serde_json::from_value(json)?;
        // Output debugging information
        debug!(
            instances = result.len(),
            "Successfully fetched cloud instances",
        );
        // Return the instances
        Ok(result)
    }

    /// Get a single cloud instance by ID
    pub async fn get_instance(&self, instance_id: &str) -> Result<CloudInstance> {
        // Output debugging information
        debug!(
            instance_id = instance_id,
            "Fetching cloud instance from SurrealDB Cloud",
        );
        // Send the request
        let response = self.get(&format!("/instances/{instance_id}")).await?;
        // Check the response status
        if !response.status().is_success() {
            let e = response.text().await?;
            error!(
                instance_id = instance_id,
                "Failed to fetch cloud instance: {e}",
            );
            return Err(anyhow::anyhow!("Failed to fetch cloud instance: {e}"));
        }
        // Parse the returned response as raw JSON
        let json: serde_json::Value = response.json().await?;
        // Parse the raw JSON into instance
        let result: CloudInstance = serde_json::from_value(json)?;
        // Output debugging information
        debug!(
            instance_id = instance_id,
            "Successfully fetched cloud instance",
        );
        // Return the instance
        Ok(result)
    }

    /// Create a cloud instance in SurrealDB Cloud
    pub async fn create_instance(
        &self,
        organization_id: &str,
        name: &str,
    ) -> Result<CloudInstance> {
        // Output debugging information
        debug!(
            instance_name = name,
            organization_id = organization_id,
            "Creating cloud instance in SurrealDB Cloud",
        );
        // Create the request
        let request = CloudCreateInstanceRequest {
            name: name.to_string(),
            organization_id: organization_id.to_string(),
        };
        // Send the request
        let response = self
            .post(
                &format!("/organizations/{organization_id}/instances"),
                &request,
            )
            .await?;
        // Check the response status
        if !response.status().is_success() {
            let e = response.text().await?;
            error!(
                organization_id = organization_id,
                instance_name = name,
                "Failed to create cloud instance: {e}",
            );
            return Err(anyhow::anyhow!("Failed to create cloud instance: {e}"));
        }
        // Parse the returned response
        let result: CloudCreateInstanceResponse = response.json().await?;
        // Output debugging information
        info!(
            instance_id = result.instance.id,
            instance_name = result.instance.name,
            instance_state = result.instance.state.as_deref(),
            "Successfully created cloud instance",
        );
        // Return the instance
        Ok(result.instance)
    }

    /// Pause a cloud instance in SurrealDB Cloud
    pub async fn pause_instance(&self, instance_id: &str) -> Result<CloudInstance> {
        // Output debugging information
        debug!(
            instance_id = instance_id,
            "Pausing cloud instance in SurrealDB Cloud",
        );
        // Send the request
        let response = self
            .post(&format!("/instances/{instance_id}/pause"), &())
            .await?;
        // Check the response status
        if !response.status().is_success() {
            let e = response.text().await?;
            error!(
                instance_id = instance_id,
                "Failed to pause cloud instance: {e}",
            );
            return Err(anyhow::anyhow!("Failed to pause cloud instance: {e}"));
        }
        // Parse the returned response as raw JSON
        let json: serde_json::Value = response.json().await?;
        // Parse the raw JSON into instance
        let result: CloudInstance = serde_json::from_value(json)?;
        // Output debugging information
        info!(
            instance_id = instance_id,
            "Successfully paused cloud instance",
        );
        // Return the instance
        Ok(result)
    }

    /// Resume a cloud instance in SurrealDB Cloud
    pub async fn resume_instance(&self, instance_id: &str) -> Result<CloudInstance> {
        // Output debugging information
        debug!(
            instance_id = instance_id,
            "Resuming cloud instance in SurrealDB Cloud",
        );
        // Send the request
        let response = self
            .post(&format!("/instances/{instance_id}/resume"), &())
            .await?;
        // Check the response status
        if !response.status().is_success() {
            let e = response.text().await?;
            error!(
                instance_id = instance_id,
                "Failed to resume cloud instance: {e}",
            );
            return Err(anyhow::anyhow!("Failed to resume cloud instance: {e}"));
        }
        // Parse the returned response as raw JSON
        let json: serde_json::Value = response.json().await?;
        // Parse the raw JSON into instance
        let result: CloudInstance = serde_json::from_value(json)?;
        // Output debugging information
        info!(
            instance_id = instance_id,
            "Successfully resumed cloud instance",
        );
        // Return the instance
        Ok(result)
    }

    /// Fetch the status for a cloud instance in SurrealDB Cloud
    pub async fn get_instance_status(&self, instance_id: &str) -> Result<CloudInstanceStatus> {
        // Output debugging information
        debug!(
            instance_id = instance_id,
            "Fetching status for cloud instance in SurrealDB Cloud",
        );
        // Send the request
        let response = self
            .get(&format!("/instances/{instance_id}/status"))
            .await?;
        // Check the response status
        if !response.status().is_success() {
            let e = response.text().await?;
            error!(
                instance_id = instance_id,
                "Failed to fetch status for cloud instance: {e}",
            );
            return Err(anyhow::anyhow!(
                "Failed to fetch status for cloud instance: {e}"
            ));
        }
        // Parse the returned response as raw JSON
        let json: serde_json::Value = response.json().await?;
        // Parse the raw JSON into instance status
        let result: CloudInstanceStatus = serde_json::from_value(json)?;
        // Output debugging information
        info!(
            instance_id = instance_id,
            phase = result.phase,
            backup_count = result.db_backups.len(),
            "Successfully fetched status for cloud instance",
        );
        // Return the instance status
        Ok(result)
    }

    /// Get authentication token for a cloud instance
    pub async fn get_instance_auth(&self, instance_id: &str) -> Result<String> {
        // Output debugging information
        debug!(
            instance_id = instance_id,
            "Fetching auth token for cloud instance",
        );
        // Send the request
        let response = self.get(&format!("/instances/{instance_id}/auth")).await?;
        // Check the response status
        if !response.status().is_success() {
            let e = response.text().await?;
            error!(
                instance_id = instance_id,
                "Failed to fetch auth token for cloud instance: {e}",
            );
            return Err(anyhow::anyhow!(
                "Failed to fetch auth token for cloud instance: {e}"
            ));
        }
        // Parse the returned response
        let result: CloudInstanceAuth = response.json().await?;
        // Output debugging information
        debug!(
            instance_id = instance_id,
            "Successfully fetched auth token for cloud instance",
        );
        // Return the auth token
        Ok(result.token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_new() {
        let client = Client::new();

        // Test that tokens are not set initially
        let auth_token = client.auth_token.try_read().unwrap();
        let refresh_token = client.refresh_token.try_read().unwrap();

        assert_eq!(*auth_token, None);
        assert_eq!(*refresh_token, None);
    }

    #[test]
    fn test_client_with_tokens() {
        let access_token = "test_access_token".to_string();
        let refresh_token = "test_refresh_token".to_string();

        let client = Client::with_tokens(access_token.clone(), refresh_token.clone());

        // Test that tokens are set correctly
        let auth_token = client.auth_token.try_read().unwrap();
        let refresh_token_guard = client.refresh_token.try_read().unwrap();

        assert_eq!(*auth_token, Some(access_token));
        assert_eq!(*refresh_token_guard, Some(refresh_token));
    }

    #[test]
    fn test_cloud_organization_deserialization() {
        // Sample API response data (simplified version of what you provided)
        let json_data = r#"
        [
            {
                "id": "069mttg269u3hd0g88man5p1co",
                "name": "Individual",
                "billing_info": true,
                "payment_info": false,
                "max_free_instances": 1,
                "max_paid_instances": 8,
                "member_count": 1,
                "user_role": "owner",
                "plan": {
                    "id": "069i2gp0kps51530vs38n951mc",
                    "name": "Start Employee",
                    "description": "Start",
                    "regions": ["aws-euw1", "aws-use1"],
                    "instance_types": [
                        {
                            "slug": "free",
                            "display_name": "free",
                            "description": "",
                            "cpu": 0.25,
                            "memory": 512,
                            "compute_units": {"min": 1, "max": 1},
                            "price_hour": 0,
                            "enabled": true,
                            "category": "free",
                            "default_storage_size": 1,
                            "max_storage_size": 1,
                            "restricted": false
                        }
                    ]
                },
                "available_plans": [
                    {
                        "id": "069i2gp0kps51530vs38n951mc",
                        "name": "Start Employee",
                        "description": "Start"
                    }
                ]
            }
        ]
        "#;

        // Try to deserialize the JSON data
        match serde_json::from_str::<Vec<CloudOrganization>>(json_data) {
            Ok(organizations) => {
                assert_eq!(organizations.len(), 1);
                let org = &organizations[0];
                assert_eq!(org.id, "069mttg269u3hd0g88man5p1co");
                assert_eq!(org.name, "Individual");
                assert_eq!(org.billing_info, Some(true));
                assert_eq!(org.payment_info, Some(false));
                assert_eq!(org.max_free_instances, Some(1));
                assert_eq!(org.max_paid_instances, Some(8));
                assert_eq!(org.member_count, Some(1));
                assert_eq!(org.user_role, Some("owner".to_string()));
                println!("✅ Successfully deserialized organization with new fields");
            }
            Err(e) => {
                panic!("❌ Failed to deserialize: {e}");
            }
        }
    }

    #[test]
    fn test_cloud_instance_deserialization() {
        // Sample API response data for instances
        let json_data = r#"
        [
            {
                "id": "069qdvg8vltknarqrtdcntjpmo",
                "name": "Test",
                "slug": "discrete-lobste",
                "version": "2.2.6",
                "available_versions": ["2.3.6"],
                "host": "discrete-lobste-069qdvg8vltknarqrtdcntjpmo.aws-euw1.surreal.cloud",
                "region": "aws-euw1",
                "type": {
                    "slug": "free",
                    "display_name": "free",
                    "description": "",
                    "cpu": 0.25,
                    "memory": 512,
                    "compute_units": {"min": 1, "max": 1},
                    "price_hour": 0,
                    "category": "free",
                    "default_storage_size": 1,
                    "max_storage_size": 1,
                    "restricted": false
                },
                "organization_id": "069mttg269u3hd0g88man5p1co",
                "compute_units": 1,
                "state": "paused",
                "storage_size": 1,
                "can_update_storage_size": false,
                "storage_size_update_cooloff_hours": 6,
                "capabilities": {
                    "allow_scripting": true,
                    "allow_guests": false,
                    "allowed_experimental": [],
                    "denied_experimental": ["*"],
                    "allowed_arbitrary_query": ["*"],
                    "denied_arbitrary_query": [],
                    "allowed_rpc_methods": ["*"],
                    "denied_rpc_methods": [],
                    "allowed_http_endpoints": ["*"],
                    "denied_http_endpoints": [],
                    "allowed_networks": [],
                    "denied_networks": ["*"],
                    "allowed_functions": ["*"],
                    "denied_functions": []
                }
            }
        ]
        "#;

        // Try to deserialize the JSON data
        match serde_json::from_str::<Vec<CloudInstance>>(json_data) {
            Ok(instances) => {
                assert_eq!(instances.len(), 1);
                let instance = &instances[0];
                assert_eq!(instance.id, "069qdvg8vltknarqrtdcntjpmo");
                assert_eq!(instance.name, "Test");
                assert_eq!(instance.slug, Some("discrete-lobste".to_string()));
                assert_eq!(instance.version, Some("2.2.6".to_string()));
                assert_eq!(instance.available_versions, Some(vec!["2.3.6".to_string()]));
                assert_eq!(
                    instance.host,
                    Some(
                        "discrete-lobste-069qdvg8vltknarqrtdcntjpmo.aws-euw1.surreal.cloud"
                            .to_string()
                    )
                );
                assert_eq!(instance.region, Some("aws-euw1".to_string()));
                assert_eq!(
                    instance.organization_id,
                    Some("069mttg269u3hd0g88man5p1co".to_string())
                );
                assert_eq!(instance.compute_units, Some(1));
                assert_eq!(instance.state, Some("paused".to_string()));
                assert_eq!(instance.storage_size, Some(1));
                assert_eq!(instance.can_update_storage_size, Some(false));
                assert_eq!(instance.storage_size_update_cooloff_hours, Some(6));
                println!("✅ Successfully deserialized instance with new fields");
            }
            Err(e) => {
                panic!("❌ Failed to deserialize: {e}");
            }
        }
    }

    #[test]
    fn test_cloud_instance_status_deserialization() {
        // Sample API response data for instance status
        let json_data = r#"
        {
            "phase": "WaitingForDeployment",
            "db_backups": [
                {
                    "snapshot_started_at": "2025-07-01T09:03:26Z",
                    "snapshot_id": "8a638067-76a7-44d9-81a4-5c4eb71a8838"
                },
                {
                    "snapshot_started_at": "2025-06-26T14:04:46Z",
                    "snapshot_id": "e9be2656-ab22-4c14-9c1c-ca8342be5150"
                },
                {
                    "snapshot_started_at": "2025-06-19T10:21:23Z",
                    "snapshot_id": "57760f45-67cc-49cc-bae9-27610c8af051"
                }
            ]
        }
        "#;

        // Try to deserialize the JSON data
        match serde_json::from_str::<CloudInstanceStatus>(json_data) {
            Ok(status) => {
                assert_eq!(status.phase, "WaitingForDeployment");
                assert_eq!(status.db_backups.len(), 3);

                let backup1 = &status.db_backups[0];
                assert_eq!(backup1.snapshot_started_at, "2025-07-01T09:03:26Z");
                assert_eq!(backup1.snapshot_id, "8a638067-76a7-44d9-81a4-5c4eb71a8838");

                let backup2 = &status.db_backups[1];
                assert_eq!(backup2.snapshot_started_at, "2025-06-26T14:04:46Z");
                assert_eq!(backup2.snapshot_id, "e9be2656-ab22-4c14-9c1c-ca8342be5150");

                let backup3 = &status.db_backups[2];
                assert_eq!(backup3.snapshot_started_at, "2025-06-19T10:21:23Z");
                assert_eq!(backup3.snapshot_id, "57760f45-67cc-49cc-bae9-27610c8af051");

                println!("✅ Successfully deserialized instance status with backups");
            }
            Err(e) => {
                panic!("❌ Failed to deserialize: {e}");
            }
        }
    }

    #[test]
    fn test_cloud_instance_auth_response_deserialization() {
        // Sample API response data for instance auth token
        let json_data = r#"
        {
            "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"
        }
        "#;

        // Try to deserialize the JSON data
        match serde_json::from_str::<CloudInstanceAuth>(json_data) {
            Ok(auth_response) => {
                assert_eq!(
                    auth_response.token,
                    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c"
                );
                println!("✅ Successfully deserialized instance auth response");
            }
            Err(e) => {
                panic!("❌ Failed to deserialize: {e}");
            }
        }
    }
}
