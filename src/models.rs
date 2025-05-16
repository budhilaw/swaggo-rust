#![allow(non_snake_case)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a complete OpenAPI 3.1.1 specification
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct OpenAPI {
    pub openapi: String,
    pub info: Info,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jsonSchemaDialect: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servers: Option<Vec<Server>>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub paths: HashMap<String, PathItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhooks: Option<HashMap<String, PathItem>>,
    pub components: Option<Components>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub security: Vec<HashMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tags: Vec<Tag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub externalDocs: Option<ExternalDocs>,
}

/// Server object used for API endpoints
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Server {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub variables: HashMap<String, ServerVariable>,
}

/// Server variable for templated server URLs
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ServerVariable {
    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
    pub default: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Components object for reusable components
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Components {
    #[serde(default)]
    pub schemas: HashMap<String, Schema>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub responses: HashMap<String, Response>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub parameters: HashMap<String, Parameter>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub examples: HashMap<String, Example>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub requestBodies: HashMap<String, RequestBody>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub headers: HashMap<String, Header>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub securitySchemes: HashMap<String, SecurityScheme>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub links: HashMap<String, Link>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub callbacks: HashMap<String, HashMap<String, PathItem>>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub pathItems: HashMap<String, PathItem>,
}

impl Components {
    // Ensure that schemas is initialized and will be serialized even if empty
    pub fn ensure_schemas_exists(&mut self) {
        // Don't add dummy schema and remove it - this doesn't prevent schemas from being skipped
        // The schemas field needs to be included in serialization even when empty
        if self.schemas.is_empty() {
            // Add a minimal schema that will remain in the output
            self.schemas.insert("Empty".to_string(), Schema::default());
        }
    }
}

/// Example object
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Example {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub externalValue: Option<String>,
}

/// Link object
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Link {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operationRef: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operationId: Option<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub parameters: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requestBody: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<Server>,
}

/// Request body object
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RequestBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub content: HashMap<String, MediaType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

/// Media type object
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MediaType {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<Schema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub examples: HashMap<String, Example>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub encoding: HashMap<String, Encoding>,
}

/// Encoding object
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Encoding {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contentType: Option<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub headers: HashMap<String, Header>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explode: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowReserved: Option<bool>,
}

/// Information about the API
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Info {
    pub title: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub termsOfService: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact: Option<Contact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<License>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// Contact information for the API
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Contact {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// License information for the API
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct License {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
}

/// External documentation for the API
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ExternalDocs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub url: String,
}

/// Tag information for API operations
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Tag {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub externalDocs: Option<ExternalDocs>,
}

/// A single path item with all its operations
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PathItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub get: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub put: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delete: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servers: Option<Vec<Server>>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub parameters: Vec<Parameter>,
}

/// An operation (endpoint) of the API
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Operation {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub externalDocs: Option<ExternalDocs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operationId: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub parameters: Vec<Parameter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requestBody: Option<RequestBody>,
    pub responses: HashMap<String, Response>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub callbacks: HashMap<String, HashMap<String, PathItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub security: Vec<HashMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub servers: Vec<Server>,
    // Added for internal use, not serialized to OpenAPI
    #[serde(skip)]
    pub consumes: Vec<String>,
    // Added for internal use, not serialized to OpenAPI
    #[serde(skip)]
    pub produces: Vec<String>,
}

/// Parameter for an operation
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "in")]
    pub in_type: String, // path, query, header, cookie
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowEmptyValue: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explode: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowReserved: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<Schema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub examples: HashMap<String, Example>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub content: HashMap<String, MediaType>,
}

/// API response
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Response {
    // The status code (not serialized to OpenAPI)
    #[serde(skip)]
    pub code: String,
    pub description: String,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub headers: HashMap<String, Header>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub content: HashMap<String, MediaType>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub links: HashMap<String, Link>,
}

/// Response header
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Header {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<Schema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub examples: HashMap<String, Example>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub content: HashMap<String, MediaType>,
}

/// Schema object updated for OpenAPI 3.1.1 with full JSON Schema 2020-12 support
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Schema {
    #[serde(rename = "$ref", skip_serializing_if = "Option::is_none")]
    pub ref_: Option<String>,
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema_: Option<String>,
    #[serde(rename = "$id", skip_serializing_if = "Option::is_none")]
    pub id_: Option<String>,
    #[serde(rename = "$anchor", skip_serializing_if = "Option::is_none")]
    pub anchor_: Option<String>,
    #[serde(rename = "$dynamicRef", skip_serializing_if = "Option::is_none")]
    pub dynamic_ref_: Option<String>,
    #[serde(rename = "$dynamicAnchor", skip_serializing_if = "Option::is_none")]
    pub dynamic_anchor_: Option<String>,
    #[serde(
        rename = "$vocabulary",
        skip_serializing_if = "HashMap::is_empty",
        default
    )]
    pub vocabulary_: HashMap<String, bool>,
    #[serde(rename = "$comment", skip_serializing_if = "Option::is_none")]
    pub comment_: Option<String>,
    #[serde(rename = "$defs", skip_serializing_if = "HashMap::is_empty", default)]
    pub defs_: HashMap<String, Box<Schema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readOnly: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub writeOnly: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub externalDocs: Option<ExternalDocs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discriminator: Option<Discriminator>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xml: Option<Xml>,

    // JSON Schema validation keywords
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<serde_json::Value>, // Can be a string or array of strings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub const_: Option<serde_json::Value>,
    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<serde_json::Value>>,

    // Number validation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multipleOf: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclusiveMaximum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclusiveMinimum: Option<f64>,

    // String validation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maxLength: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minLength: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,

    // Array validation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maxItems: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minItems: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uniqueItems: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maxContains: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minContains: Option<u64>,

    // Object validation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maxProperties: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minProperties: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub properties: HashMap<String, Box<Schema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patternProperties: Option<HashMap<String, Box<Schema>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additionalProperties: Option<serde_json::Value>, // Can be a boolean or Schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub propertyNames: Option<Box<Schema>>,

    // Composition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allOf: Option<Vec<Schema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anyOf: Option<Vec<Schema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oneOf: Option<Vec<Schema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not: Option<Box<Schema>>,

    // Array item validation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<Schema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contains: Option<Box<Schema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefixItems: Option<Vec<Schema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unevaluatedItems: Option<serde_json::Value>, // Can be a boolean or Schema

    // Object unevaluated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unevaluatedProperties: Option<serde_json::Value>, // Can be a boolean or Schema

    // Conditional schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub if_: Option<Box<Schema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub then: Option<Box<Schema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub else_: Option<Box<Schema>>,

    // Content validation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contentEncoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contentMediaType: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contentSchema: Option<Box<Schema>>,
}

/// XML object
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Xml {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attribute: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrapped: Option<bool>,
}

/// Discriminator object for Schema composition
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Discriminator {
    pub propertyName: String,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub mapping: HashMap<String, String>,
}

/// Security scheme object
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SecurityScheme {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "in")]
    pub in_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bearerFormat: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flows: Option<OAuthFlows>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openIdConnectUrl: Option<String>,
}

/// OAuth Flows Object
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct OAuthFlows {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implicit: Option<OAuthFlow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<OAuthFlow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clientCredentials: Option<OAuthFlow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorizationCode: Option<OAuthFlow>,
}

/// OAuth Flow Object
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct OAuthFlow {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorizationUrl: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenUrl: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refreshUrl: Option<String>,
    pub scopes: HashMap<String, String>,
}

/// Represents a parsed API operation from Go code comments
#[derive(Debug, Clone)]
pub struct ParsedOperation {
    pub path: String,
    pub method: String,
    pub operation: Operation,
}

/// Represents parsed general API info from Go code comments
#[derive(Debug, Clone)]
pub struct ParsedApiInfo {
    pub info: Info,
    pub servers: Vec<Server>,
    #[allow(dead_code)]
    pub security: Vec<HashMap<String, Vec<String>>>,
    pub security_definitions: HashMap<String, SecurityScheme>,
    pub tags: Vec<Tag>,
    pub external_docs: Option<ExternalDocs>,
    // Legacy fields to maintain compatibility with Swagger 2.0 parsers
    pub host: Option<String>,
    pub base_path: Option<String>,
    pub schemes: Vec<String>,
    pub consumes: Vec<String>,
    pub produces: Vec<String>,
}

impl Default for ParsedApiInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl ParsedApiInfo {
    pub fn new() -> Self {
        Self {
            info: Info {
                title: String::new(),
                version: String::new(),
                description: None,
                termsOfService: None,
                contact: None,
                license: None,
                summary: None,
            },
            servers: Vec::new(),
            host: None,
            base_path: None,
            schemes: Vec::new(),
            consumes: Vec::new(),
            produces: Vec::new(),
            security_definitions: HashMap::new(),
            security: Vec::new(),
            tags: Vec::new(),
            external_docs: None,
        }
    }
}
