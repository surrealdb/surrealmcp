use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, error, info, trace};

const CLOUD_API_BASE_URL: &str = "https://api.cloud.surrealdb.com/api/v1";

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
    pub slug: String,
    pub created_at: String,
    pub updated_at: String,
}

/// A cloud instance in SurrealDB Cloud
#[derive(Debug, Serialize, Deserialize)]
pub struct CloudInstance {
    pub id: String,
    pub name: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// A response from signing in to SurrealDB Cloud
#[derive(Debug, Serialize, Deserialize)]
pub struct CloudSignInResponse {
    pub id: String,
    pub token: String,
}

/// A response from listing organizations in SurrealDB Cloud
#[derive(Debug, Serialize, Deserialize)]
pub struct CloudOrganizationsResponse {
    pub organizations: Vec<CloudOrganization>,
}

/// A response from listing cloud instances
#[derive(Debug, Serialize, Deserialize)]
pub struct CloudInstancesResponse {
    pub instances: Vec<CloudInstance>,
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
        // Parse the returned response
        let result: CloudOrganizationsResponse = response.json().await?;
        // Output debugging information
        debug!(
            organisations = result.organizations.len(),
            "Successfully fetched organizations",
        );
        // Return the organizations
        Ok(result.organizations)
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
        // Parse the returned response
        let result: CloudInstancesResponse = response.json().await?;
        // Output debugging information
        debug!(
            instances = result.instances.len(),
            "Successfully fetched cloud instances",
        );
        // Return the instances
        Ok(result.instances)
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
            instance_status = result.instance.status,
            "Successfully created cloud instance",
        );
        // Return the instance
        Ok(result.instance)
    }

    /// Pause a cloud instance in SurrealDB Cloud
    pub async fn pause_instance(&self, instance_id: &str) -> Result<()> {
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
        // Output debugging information
        info!(
            instance_id = instance_id,
            "Successfully paused cloud instance",
        );
        // Return nothing
        Ok(())
    }

    /// Resume a cloud instance in SurrealDB Cloud
    pub async fn resume_instance(&self, instance_id: &str) -> Result<()> {
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
        // Output debugging information
        info!(
            instance_id = instance_id,
            "Successfully resumed cloud instance",
        );
        // Return nothing
        Ok(())
    }

    /// Fetch the status for a cloud instance in SurrealDB Cloud
    pub async fn get_instance_status(&self, instance_id: &str) -> Result<()> {
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
        // Output debugging information
        info!(
            instance_id = instance_id,
            "Successfully fetched status for cloud instance",
        );
        // Return nothing
        Ok(())
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
}
