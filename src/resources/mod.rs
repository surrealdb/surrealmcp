use rmcp::model::{Annotated, RawResource, ReadResourceResult, Resource, ResourceContents};

// Trait and provider-based resource registry (similar to prompts)
pub trait ResourceProvider {
    /// Get the resource URI
    fn uri(&self) -> &'static str;

    /// Get the resource name
    fn name(&self) -> &'static str;

    /// Get the resource description
    fn description(&self) -> &'static str;

    /// Get the resource MIME type
    fn mime_type(&self) -> &'static str;

    /// Get the resource content
    fn content(&self) -> String;

    /// Get the resource metadata
    fn meta(&self) -> Resource {
        let size = self.content().len() as u32;
        let raw = RawResource {
            size: Some(size),
            uri: self.uri().to_string(),
            name: self.name().to_string(),
            mime_type: Some(self.mime_type().to_string()),
            description: Some(self.description().to_string()),
        };
        Annotated::new(raw, None)
    }

    fn read(&self) -> ReadResourceResult {
        ReadResourceResult {
            contents: vec![ResourceContents::text(self.content(), self.uri())],
        }
    }
}

// Instructions resource
pub struct InstructionsResource;

impl ResourceProvider for InstructionsResource {
    fn uri(&self) -> &'static str {
        "surrealmcp://instructions"
    }

    fn name(&self) -> &'static str {
        "SurrealMCP Instructions"
    }

    fn mime_type(&self) -> &'static str {
        "text/markdown"
    }

    fn description(&self) -> &'static str {
        "Full instructions and guidelines for the SurrealDB MCP server"
    }

    fn content(&self) -> String {
        include_str!("../../instructions.md").to_string()
    }
}

/// Registry of all available resources
pub struct ResourceRegistry;

impl ResourceRegistry {
    /// Get all available resource providers
    pub fn get_providers() -> Vec<Box<dyn ResourceProvider>> {
        vec![Box::new(InstructionsResource)]
    }

    /// Find a resource provider by URI
    pub fn find_by_uri(uri: &str) -> Option<Box<dyn ResourceProvider>> {
        Self::get_providers().into_iter().find(|p| p.uri() == uri)
    }
}

/// List all available resources
pub fn list_resources() -> Vec<Resource> {
    ResourceRegistry::get_providers()
        .into_iter()
        .map(|p| p.meta())
        .collect()
}

pub fn read_resource(uri: &str) -> Option<ReadResourceResult> {
    ResourceRegistry::find_by_uri(uri).map(|provider| provider.read())
}
