use rmcp::model::{Prompt, PromptArgument, PromptMessage, PromptMessageRole};
use serde_json::{Map, Value};

/// Trait that defines the behavior for generating prompt output
pub trait PromptGenerator {
    /// Get the prompt name
    fn name(&self) -> &'static str;

    /// Get the prompt summary
    fn summary(&self) -> &'static str;

    /// Get the prompt description
    fn description(&self) -> &'static str;

    /// Get the prompt arguments
    fn arguments(&self) -> Vec<PromptArgument>;

    /// Generate the prompt messages based on the provided arguments
    fn generate(&self, arguments: Option<Map<String, Value>>) -> Vec<PromptMessage>;
}

/// Database Query Assistant prompt
pub struct DatabaseQueryAssistant;

impl PromptGenerator for DatabaseQueryAssistant {
    fn name(&self) -> &'static str {
        "database_query_assistant"
    }

    fn summary(&self) -> &'static str {
        "Database query assistant prompt"
    }

    fn description(&self) -> &'static str {
        "A helpful assistant for writing and optimizing SurrealQL queries"
    }

    fn arguments(&self) -> Vec<PromptArgument> {
        vec![
            PromptArgument {
                name: "query_type".to_string(),
                description: Some(
                    "The type of query (SELECT, CREATE, UPDATE, DELETE, etc.)".to_string(),
                ),
                required: Some(true),
            },
            PromptArgument {
                name: "table_name".to_string(),
                description: Some("The table name to query".to_string()),
                required: Some(false),
            },
            PromptArgument {
                name: "requirements".to_string(),
                description: Some("Specific requirements or constraints for the query".to_string()),
                required: Some(false),
            },
        ]
    }

    fn generate(&self, arguments: Option<Map<String, Value>>) -> Vec<PromptMessage> {
        // Get the arguments
        let query_type = arguments
            .as_ref()
            .and_then(|args| args.get("query_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("SELECT");
        let table_name = arguments
            .as_ref()
            .and_then(|args| args.get("table_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("your_table");
        let requirements = arguments
            .as_ref()
            .and_then(|args| args.get("requirements"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        // Format the requirements
        let requirements = if requirements.is_empty() {
            "".to_string()
        } else {
            format!("Requirements: {requirements}")
        };
        // Return the prompt messages
        vec![
            PromptMessage::new_text(
                PromptMessageRole::User,
                format!(
                    "You are a SurrealQL expert assistant. Help me write a {query_type} query for the '{table_name}' table. {requirements}",
                ),
            ),
            PromptMessage::new_text(
                PromptMessageRole::Assistant,
                "I'll help you write an optimized SurrealQL query. Let me break this down step by step and provide you with the best approach for your use case.".to_string(),
            ),
        ]
    }
}

/// Data Modeling Expert prompt
pub struct DataModelingExpert;

impl PromptGenerator for DataModelingExpert {
    fn name(&self) -> &'static str {
        "data_modeling_expert"
    }

    fn summary(&self) -> &'static str {
        "Data modeling expert prompt"
    }

    fn description(&self) -> &'static str {
        "An expert assistant for designing and optimizing SurrealDB data models"
    }

    fn arguments(&self) -> Vec<PromptArgument> {
        vec![
            PromptArgument {
                name: "use_case".to_string(),
                description: Some("The use case or application domain (e.g., social network, e-commerce, analytics)".to_string()),
                required: Some(true),
            },
            PromptArgument {
                name: "data_types".to_string(),
                description: Some("The types of data to be stored (users, posts, transactions, etc.)".to_string()),
                required: Some(false),
            },
            PromptArgument {
                name: "scale_requirements".to_string(),
                description: Some("Scale requirements (small, medium, large, enterprise)".to_string()),
                required: Some(false),
            },
        ]
    }

    fn generate(&self, arguments: Option<Map<String, Value>>) -> Vec<PromptMessage> {
        // Get the arguments
        let use_case = arguments
            .as_ref()
            .and_then(|args| args.get("use_case"))
            .and_then(|v| v.as_str())
            .unwrap_or("general application");
        let data_types = arguments
            .as_ref()
            .and_then(|args| args.get("data_types"))
            .and_then(|v| v.as_str())
            .unwrap_or("users and content");
        let scale_requirements = arguments
            .as_ref()
            .and_then(|args| args.get("scale_requirements"))
            .and_then(|v| v.as_str())
            .unwrap_or("medium");
        // Return the prompt messages
        vec![
            PromptMessage::new_text(
                PromptMessageRole::User,
                format!(
                    "You are a SurrealDB data modeling expert. Help me design an optimal data model for a {use_case} application that needs to handle {data_types}. The scale requirements are: {scale_requirements}."
                ),
            ),
            PromptMessage::new_text(
                PromptMessageRole::Assistant,
                "I'll help you design an optimal SurrealDB data model. Let me analyze your requirements and provide a comprehensive solution with proper table structures, relationships, and indexing strategies.".to_string(),
            ),
        ]
    }
}

/// SurrealQL Guide prompt (best-practice system + examples)
pub struct SurrealQlGuide;

impl PromptGenerator for SurrealQlGuide {
    fn name(&self) -> &'static str {
        "surrealql_guide"
    }

    fn summary(&self) -> &'static str {
        "Comprehensive SurrealQL writing guide"
    }

    fn description(&self) -> &'static str {
        "A prompt that provides best practices and examples for writing correct and efficient SurrealQL"
    }

    fn arguments(&self) -> Vec<PromptArgument> {
        vec![
            PromptArgument {
                name: "task".to_string(),
                description: Some(
                    "Brief description of what you need to do in SurrealQL".to_string(),
                ),
                required: Some(false),
            },
            PromptArgument {
                name: "schema".to_string(),
                description: Some(
                    "Optional schema or table context relevant to the task".to_string(),
                ),
                required: Some(false),
            },
        ]
    }

    fn generate(&self, arguments: Option<Map<String, Value>>) -> Vec<PromptMessage> {
        let task = arguments
            .as_ref()
            .and_then(|args| args.get("task"))
            .and_then(|v| v.as_str())
            .unwrap_or("Write SurrealQL for my task");
        let schema = arguments
            .as_ref()
            .and_then(|args| args.get("schema"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let user_text = if schema.is_empty() {
            task.to_string()
        } else {
            format!("{task}\n\nSchema/context:\n{schema}")
        };

        let system_guide = r#"You are a SurrealDB expert. Produce correct, safe, and efficient SurrealQL.

Best practices:
- Prefer parameterized queries with $name variables; avoid string concatenation.
- Use explicit WHERE filters; never update/delete entire tables unless user explicitly asks.
- For CREATE/UPSERT/UPDATE use CONTENT/MERGE/REPLACE appropriately.
- Use SPLIT ON for arrays, GROUP BY for aggregations, and ORDER BY with explicit direction.
- For relations, use RELATE a->edge->b, and optionally CONTENT for edge properties.
- Keep queries idempotent when possible; explain assumptions.
- Return only necessary fields using SELECT field projection when asked to optimize.

Examples:
-- Parameterized select
SELECT id, name FROM user WHERE age > $min_age AND name CONTAINS $name ORDER BY age DESC LIMIT 10;

-- Create with content
CREATE user CONTENT { id: user:alice, name: $name, age: $age };

-- Update merge vs replace
UPDATE user:alice MERGE { preferences: { theme: "dark" } };
UPDATE user:alice REPLACE { name: "Alice", age: 30 };

-- Relate with properties
RELATE user:alice->follows->user:bob CONTENT { since: time::now() };

-- Upsert pattern
UPSERT user:alice CONTENT { name: $name, age: $age };

If details are missing, ask concise clarifying questions before executing risky operations. Provide the final SurrealQL first, then a brief explanation."#;

        vec![
            PromptMessage::new_text(PromptMessageRole::Assistant, system_guide.to_string()),
            PromptMessage::new_text(PromptMessageRole::User, user_text),
        ]
    }
}

/// Registry of all available prompts
pub struct PromptRegistry;

impl PromptRegistry {
    /// Get all available prompt generators
    pub fn get_generators() -> Vec<Box<dyn PromptGenerator>> {
        vec![
            Box::new(DatabaseQueryAssistant),
            Box::new(DataModelingExpert),
            Box::new(SurrealQlGuide),
        ]
    }

    /// Find a prompt generator by name
    pub fn find_by_name(name: &str) -> Option<Box<dyn PromptGenerator>> {
        Self::get_generators()
            .into_iter()
            .find(|generator| generator.name() == name)
    }
}

/// List all available prompts
pub fn list_prompts() -> Vec<Prompt> {
    PromptRegistry::get_generators()
        .into_iter()
        .map(|generator| Prompt {
            name: generator.name().to_string(),
            description: Some(generator.description().to_string()),
            arguments: Some(generator.arguments()),
        })
        .collect()
}

/// Get a specific prompt by name with arguments
pub fn get_prompt_with_arguments(
    name: &str,
    arguments: Option<Map<String, Value>>,
) -> Option<(String, Vec<PromptMessage>)> {
    PromptRegistry::find_by_name(name).map(|generator| {
        (
            generator.summary().to_string(),
            generator.generate(arguments),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_generator_trait() {
        let db_assistant = DatabaseQueryAssistant;
        assert_eq!(db_assistant.name(), "database_query_assistant");
        assert_eq!(db_assistant.summary(), "Database query assistant prompt");
        assert_eq!(
            db_assistant.description(),
            "A helpful assistant for writing and optimizing SurrealQL queries"
        );

        let args = db_assistant.arguments();
        assert_eq!(args.len(), 3);
        assert_eq!(args[0].name, "query_type");
        assert_eq!(args[1].name, "table_name");
        assert_eq!(args[2].name, "requirements");
    }

    #[test]
    fn test_prompt_registry() {
        let generators = PromptRegistry::get_generators();
        assert_eq!(generators.len(), 2);

        let db_generator = PromptRegistry::find_by_name("database_query_assistant");
        assert!(db_generator.is_some());

        let unknown_generator = PromptRegistry::find_by_name("unknown_prompt");
        assert!(unknown_generator.is_none());
    }

    #[test]
    fn test_get_available_prompts() {
        let prompts = list_prompts();
        assert_eq!(prompts.len(), 2);

        let prompt_names: Vec<&str> = prompts.iter().map(|p| p.name.as_str()).collect();
        assert!(prompt_names.contains(&"database_query_assistant"));
        assert!(prompt_names.contains(&"data_modeling_expert"));
    }

    #[test]
    fn test_get_prompt_with_arguments() {
        let mut args = Map::new();
        args.insert(
            "query_type".to_string(),
            Value::String("SELECT".to_string()),
        );
        args.insert("table_name".to_string(), Value::String("users".to_string()));

        let result = get_prompt_with_arguments("database_query_assistant", Some(args));
        assert!(result.is_some());

        let (description, messages) = result.unwrap();
        assert_eq!(description, "Database query assistant prompt");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, PromptMessageRole::User);
        assert_eq!(messages[1].role, PromptMessageRole::Assistant);
    }
}
