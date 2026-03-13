use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentsManifest {
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    #[serde(default)]
    pub endpoints: Vec<Endpoint>,
    pub authentication: Option<Authentication>,
    pub rate_limits: Option<RateLimits>,
    pub contact: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Capability {
    pub name: String,
    pub description: String,
    pub input_schema: Option<serde_json::Value>,
    pub output_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub examples: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Endpoint {
    pub path: String,
    pub method: String,
    pub description: String,
    #[serde(default)]
    pub parameters: Vec<Parameter>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Parameter {
    pub name: String,
    pub r#type: String,
    pub required: bool,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Authentication {
    pub r#type: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RateLimits {
    pub requests_per_minute: Option<u32>,
    pub requests_per_day: Option<u32>,
    pub notes: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct RepoContext {
    pub name: String,
    pub readme: Option<String>,
    pub source_files: Vec<SourceFile>,
    pub openapi_spec: Option<String>,
    pub package_manifest: Option<String>,
    pub existing_agents_md: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SourceFile {
    pub path: String,
    pub language: Language,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Language {
    Rust,
    Python,
    TypeScript,
    JavaScript,
    Go,
    Other(String),
}

impl Language {
    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "rs" => Language::Rust,
            "py" => Language::Python,
            "ts" => Language::TypeScript,
            "js" => Language::JavaScript,
            "go" => Language::Go,
            other => Language::Other(other.to_string()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub endpoint_results: Vec<EndpointCheckResult>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EndpointCheckResult {
    pub endpoint: String,
    pub reachable: bool,
    pub status_code: Option<u16>,
    pub error: Option<String>,
}
