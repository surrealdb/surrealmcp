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
            title: Some(self.name().to_string()),
            mime_type: Some(self.mime_type().to_string()),
            description: Some(self.description().to_string()),
            icons: None,
            meta: None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instructions_resource() {
        let resource = InstructionsResource;
        assert_eq!(resource.uri(), "surrealmcp://instructions");
        assert_eq!(resource.name(), "SurrealMCP Instructions");
        assert_eq!(resource.mime_type(), "text/markdown");
        assert!(resource.description().contains("SurrealDB MCP server"));
        assert!(resource.content().contains("# SurrealDB MCP Server"));
    }

    #[test]
    fn test_resource_provider_trait_defaults() {
        struct MockResource;
        impl ResourceProvider for MockResource {
            fn uri(&self) -> &'static str {
                "test://uri"
            }
            fn name(&self) -> &'static str {
                "Test Name"
            }
            fn description(&self) -> &'static str {
                "Test Description"
            }
            fn mime_type(&self) -> &'static str {
                "text/plain"
            }
            fn content(&self) -> String {
                "Test Content".to_string()
            }
        }

        let resource = MockResource;

        // Test meta() default implementation
        let meta = resource.meta();
        assert_eq!(meta.uri, "test://uri");
        assert_eq!(meta.name, "Test Name");
        assert_eq!(meta.mime_type, Some("text/plain".to_string()));
        assert_eq!(meta.description, Some("Test Description".to_string()));
        assert_eq!(meta.size, Some(12)); // "Test Content".len()

        // Test read() default implementation
        let read_result = resource.read();
        assert_eq!(read_result.contents.len(), 1);
        match &read_result.contents[0] {
            ResourceContents::Text(text) => {
                assert_eq!(text.text, "Test Content");
                assert_eq!(text.uri, "test://uri");
            }
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_resource_registry() {
        let providers = ResourceRegistry::get_providers();
        assert!(!providers.is_empty());
        assert!(providers.iter().any(|p| p.uri() == "surrealmcp://instructions"));

        let found = ResourceRegistry::find_by_uri("surrealmcp://instructions");
        assert!(found.is_some());
        assert_eq!(found.unwrap().uri(), "surrealmcp://instructions");

        let not_found = ResourceRegistry::find_by_uri("surrealmcp://non-existent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_public_functions() {
        // Test list_resources
        let resources = list_resources();
        assert!(!resources.is_empty());
        assert!(resources.iter().any(|r| r.uri == "surrealmcp://instructions"));

        // Test read_resource
        let result = read_resource("surrealmcp://instructions");
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.contents.len(), 1);
        match &result.contents[0] {
            ResourceContents::Text(text) => {
                assert_eq!(text.uri, "surrealmcp://instructions");
                assert!(text.text.contains("# SurrealDB MCP Server"));
            }
            _ => panic!("Expected text content"),
        }

        let not_found = read_resource("surrealmcp://non-existent");
        assert!(not_found.is_none());
    }
}
