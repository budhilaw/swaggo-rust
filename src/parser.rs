use anyhow::{Context, Result};
use log::{debug, warn};
use once_cell::sync::Lazy;
use regex::Regex;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};
use thiserror::Error;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;

#[allow(non_snake_case)]
use crate::models::{
    Contact, ExternalDocs, License, MediaType, OAuthFlows, Operation, Parameter, ParsedApiInfo, ParsedOperation,
    RequestBody, Response, Schema, SecurityScheme, Server,
};

static ANNOTATION_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"//\s*@(\w+)(?:\.([\w.]+))?\s+(.+)$").unwrap()
});

static MULTI_LINE_DESCRIPTION_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"//\s*(.+)$").unwrap()
});

static ROUTER_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"/(.+?)\s+\[(\w+)]$").unwrap()
});

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("Failed to read file: {0}")]
    IOError(#[from] std::io::Error),
    
    #[error("Failed to parse annotation: {0}")]
    #[allow(dead_code)]
    AnnotationParseError(String),
    
    #[error("Invalid router format: {0}")]
    RouterParseError(String),

    #[error("Invalid parameter format: {0}")]
    ParameterParseError(String),
    
    #[error("Invalid response format: {0}")]
    ResponseParseError(String),
    
    #[error("Invalid security format: {0}")]
    SecurityParseError(String),
    
    #[error("Invalid general API info: {0}")]
    #[allow(dead_code)]
    GeneralApiInfoError(String),
    
    #[error("Invalid server format: {0}")]
    ServerParseError(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnnotationType {
    // General API Info
    Title,
    Version,
    Description,
    Summary,
    TermsOfService,
    Contact,
    License,
    
    // Servers (OpenAPI 3.x)
    Server,
    
    // Legacy (Swagger 2.0)
    Host,
    BasePath,
    Accept,
    Produce,
    Schemes,
    
    // Tags
    Tag,
    
    // Security
    SecurityDefinitions,
    SecurityScheme,
    
    // External Docs
    ExternalDocs,
    
    // Operation Annotations
    Id,
    Tags,
    Router,
    DeprecatedRouter,
    Param,
    RequestBody,
    Security,
    Response,
    Header,
    Deprecated,
    
    // Unknown
    Unknown(String),
}

impl From<&str> for AnnotationType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "title" => Self::Title,
            "version" => Self::Version,
            "description" => Self::Description,
            "summary" => Self::Summary,
            "termsofservice" => Self::TermsOfService,
            "contact" => Self::Contact,
            "license" => Self::License,
            "server" => Self::Server,
            "host" => Self::Host,
            "basepath" => Self::BasePath,
            "accept" => Self::Accept,
            "produce" => Self::Produce,
            "schemes" => Self::Schemes,
            "tag" => Self::Tag,
            "securitydefinitions" => Self::SecurityDefinitions,
            "securityscheme" => Self::SecurityScheme,
            "externaldocs" => Self::ExternalDocs,
            "id" => Self::Id,
            "tags" => Self::Tags,
            "router" => Self::Router,
            "deprecatedrouter" => Self::DeprecatedRouter,
            "param" => Self::Param,
            "requestbody" => Self::RequestBody,
            "security" => Self::Security,
            "response" => Self::Response,
            "success" => Self::Response,  // For backward compatibility
            "failure" => Self::Response,  // For backward compatibility
            "header" => Self::Header,
            "deprecated" => Self::Deprecated,
            _ => Self::Unknown(s.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub annotation_type: AnnotationType,
    pub attribute: Option<String>,
    pub value: String,
}

pub struct GoParser;

#[derive(Debug, Clone, Default)]
pub struct ImportInfo {
    pub alias: String,
    pub path: String,
    pub file_path: Option<PathBuf>,
}

#[allow(non_snake_case)]
impl GoParser {
    pub fn new() -> Self {
        Self
    }
    
    pub fn parse_general_api_info(&self, file_path: impl AsRef<Path>) -> Result<ParsedApiInfo> {
        let file_path = file_path.as_ref();
        debug!("Parsing general API info from file: {:?}", file_path);
        
        let file = File::open(file_path)
            .context(format!("Failed to open file: {:?}", file_path))?;
        
        let reader = BufReader::new(file);
        let mut api_info = ParsedApiInfo::new();
        let mut contact = Contact::default();
        let mut license = License::default();
        let mut in_doc_comment = false;
        let mut description_buffer = String::new();
        let mut current_server: Option<Server> = None;
        let mut current_tag_name: Option<String> = None;
        
        for line in reader.lines() {
            let line = line?;
            
            // Check for comment annotations
            if let Some(captures) = ANNOTATION_REGEX.captures(&line) {
                let annotation_type_str = captures.get(1).unwrap().as_str();
                let attribute = captures.get(2).map(|m| m.as_str().to_string());
                let value = captures.get(3).unwrap().as_str().to_string();
                
                let annotation = Annotation {
                    annotation_type: AnnotationType::from(annotation_type_str),
                    attribute,
                    value,
                };
                
                match annotation.annotation_type {
                    AnnotationType::Title => {
                        api_info.info.title = annotation.value;
                    },
                    AnnotationType::Version => {
                        api_info.info.version = annotation.value;
                    },
                    AnnotationType::Description => {
                        if !in_doc_comment {
                            api_info.info.description = Some(annotation.value);
                        } else {
                            description_buffer.push_str(&annotation.value);
                            description_buffer.push('\n');
                        }
                    },
                    AnnotationType::Summary => {
                        api_info.info.summary = Some(annotation.value);
                    },
                    AnnotationType::TermsOfService => {
                        api_info.info.termsOfService = Some(annotation.value);
                    },
                    AnnotationType::Contact => {
                        if let Some(attribute) = annotation.attribute {
                            match attribute.as_str() {
                                "name" => contact.name = Some(annotation.value),
                                "url" => contact.url = Some(annotation.value),
                                "email" => contact.email = Some(annotation.value),
                                _ => warn!("Unknown contact attribute: {}", attribute),
                            }
                        }
                    },
                    AnnotationType::License => {
                        if let Some(attribute) = annotation.attribute {
                            match attribute.as_str() {
                                "name" => license.name = annotation.value,
                                "url" => license.url = Some(annotation.value),
                                "identifier" => license.identifier = Some(annotation.value),
                                _ => warn!("Unknown license attribute: {}", attribute),
                            }
                        }
                    },
                    AnnotationType::Server => {
                        if let Some(attribute) = annotation.attribute {
                            match attribute.as_str() {
                                "url" => {
                                    // Start a new server if we don't have one yet
                                    if current_server.is_none() {
                                        current_server = Some(Server {
                                            url: annotation.value,
                                            ..Default::default()
                                        });
                                    } else if let Some(server) = &mut current_server {
                                        server.url = annotation.value;
                                    }
                                },
                                "description" => {
                                    if let Some(server) = &mut current_server {
                                        server.description = Some(annotation.value);
                                        
                                        // Add the server to the list and reset
                                        api_info.servers.push(server.clone());
                                        current_server = None;
                                    } else {
                                        // Start a new server with just the description
                                        current_server = Some(Server {
                                            description: Some(annotation.value),
                                            ..Default::default()
                                        });
                                    }
                                },
                                _ => warn!("Unknown server attribute: {}", attribute),
                            }
                        } else {
                            return Err(ParserError::ServerParseError(
                                "Server annotation requires an attribute".to_string(),
                            ).into());
                        }
                    },
                    AnnotationType::Host => {
                        // Legacy Swagger 2.0 - store for later conversion to server
                        api_info.host = Some(annotation.value);
                    },
                    AnnotationType::BasePath => {
                        // Legacy Swagger 2.0 - store for later conversion to server
                        api_info.base_path = Some(annotation.value);
                    },
                    AnnotationType::Accept => {
                        // Legacy Swagger 2.0 consumes
                        api_info.consumes.push(self.normalize_mime_type(&annotation.value));
                    },
                    AnnotationType::Produce => {
                        // Legacy Swagger 2.0 produces
                        api_info.produces.push(self.normalize_mime_type(&annotation.value));
                    },
                    AnnotationType::Schemes => {
                        // Legacy Swagger 2.0 schemes
                        annotation.value.split_whitespace().for_each(|scheme| {
                            api_info.schemes.push(scheme.to_string());
                        });
                    },
                    AnnotationType::Tag => {
                        if let Some(attribute) = annotation.attribute {
                            match attribute.as_str() {
                                "name" => {
                                    current_tag_name = Some(annotation.value.clone());
                                    
                                    // Add the tag with just the name for now
                                    let mut found = false;
                                    for tag in &mut api_info.tags {
                                        if tag.name == annotation.value {
                                            found = true;
                                            break;
                                        }
                                    }
                                    
                                    if !found {
                                        api_info.tags.push(crate::models::Tag {
                                            name: annotation.value,
                                            description: None,
                                            externalDocs: None,
                                        });
                                    }
                                },
                                "description" => {
                                    if let Some(tag_name) = &current_tag_name {
                                        for tag in &mut api_info.tags {
                                            if &tag.name == tag_name {
                                                tag.description = Some(annotation.value);
                                                break;
                                            }
                                        }
                                    } else {
                                        warn!("Tag description provided without a name");
                                    }
                                },
                                _ => warn!("Unknown tag attribute: {}", attribute),
                            }
                        } else {
                            // For backward compatibility, treat as a tag name
                            api_info.tags.push(crate::models::Tag {
                                name: annotation.value,
                                description: None,
                                externalDocs: None,
                            });
                        }
                    },
                    AnnotationType::SecurityDefinitions => {
                        if let Some(attribute) = annotation.attribute {
                            self.parse_security_definition(&mut api_info, &attribute, &annotation.value)?;
                        } else {
                            return Err(ParserError::SecurityParseError(
                                "SecurityDefinitions annotation requires an attribute".to_string(),
                            ).into());
                        }
                    },
                    AnnotationType::SecurityScheme => {
                        if let Some(attribute) = annotation.attribute {
                            // Format: securityScheme.[scheme_name].[property]
                            let parts: Vec<&str> = attribute.split('.').collect();
                            if parts.len() >= 2 {
                                let scheme_name = parts[0];
                                let property = parts[1];
                                
                                if let Some(scheme) = api_info.security_definitions.get_mut(scheme_name) {
                                    match property {
                                        "description" => {
                                            scheme.description = Some(annotation.value.clone());
                                        },
                                        "in" => {
                                            scheme.in_type = Some(annotation.value.clone());
                                        },
                                        "name" => {
                                            scheme.name = Some(annotation.value.clone());
                                        },
                                        // Add other properties as needed
                                        _ => warn!("Unknown security scheme property: {}", property),
                                    }
                                } else {
                                    warn!("Security scheme not found: {}", scheme_name);
                                }
                            }
                        }
                    },
                    AnnotationType::Security => {
                        // Parse global security requirement
                        // Format: [name] [scopes...]
                        let parts: Vec<&str> = annotation.value.splitn(2, ' ').collect();
                        let security_name = parts[0];
                        
                        let mut security_requirement = std::collections::HashMap::new();
                        if parts.len() > 1 {
                            let scopes: Vec<String> = parts[1]
                                .split_whitespace()
                                .map(|s| s.to_string())
                                .collect();
                            security_requirement.insert(security_name.to_string(), scopes);
                        } else {
                            security_requirement.insert(security_name.to_string(), Vec::new());
                        }
                        
                        api_info.security.push(security_requirement);
                    },
                    AnnotationType::ExternalDocs => {
                        if let Some(attribute) = annotation.attribute {
                            match attribute.as_str() {
                                "description" => {
                                    if let Some(docs) = &mut api_info.external_docs {
                                        docs.description = Some(annotation.value);
                                    } else {
                                        api_info.external_docs = Some(ExternalDocs {
                                            description: Some(annotation.value),
                                            url: String::new(),
                                        });
                                    }
                                },
                                "url" => {
                                    if let Some(docs) = &mut api_info.external_docs {
                                        docs.url = annotation.value;
                                    } else {
                                        api_info.external_docs = Some(ExternalDocs {
                                            description: None,
                                            url: annotation.value,
                                        });
                                    }
                                },
                                _ => warn!("Unknown external docs attribute: {}", attribute),
                            }
                        }
                    },
                    _ => {}
                }
            } else if let Some(captures) = MULTI_LINE_DESCRIPTION_REGEX.captures(&line) {
                if in_doc_comment {
                    let comment_text = captures.get(1).unwrap().as_str().trim();
                    description_buffer.push_str(comment_text);
                    description_buffer.push('\n');
                }
            } else {
                // End of comment block
                if in_doc_comment && !description_buffer.is_empty() {
                    api_info.info.description = Some(description_buffer.trim().to_string());
                    description_buffer.clear();
                    in_doc_comment = false;
                }
            }
            
            // Check if we're starting a doc comment block
            if line.trim_start().starts_with("/*") {
                in_doc_comment = true;
                description_buffer.clear();
            }
            
            // Check if we're ending a doc comment block
            if line.trim_end().ends_with("*/") {
                in_doc_comment = false;
                if !description_buffer.is_empty() {
                    api_info.info.description = Some(description_buffer.trim().to_string());
                    description_buffer.clear();
                }
            }
        }
        
        // Add contact info if any fields were set
        if contact.name.is_some() || contact.url.is_some() || contact.email.is_some() {
            api_info.info.contact = Some(contact);
        }
        
        // Add license info if the name was set
        if license.name != String::new() {
            api_info.info.license = Some(license);
        }
        
        // Add the current server if there is one
        if let Some(server) = current_server {
            if !server.url.is_empty() {
                api_info.servers.push(server);
            }
        }
        
        // Create a default server from legacy host/basePath/schemes if no servers were defined
        if api_info.servers.is_empty() && api_info.host.is_some() {
            for scheme in &api_info.schemes {
                let url = format!(
                    "{}://{}{}",
                    scheme,
                    api_info.host.as_ref().unwrap(),
                    api_info.base_path.as_deref().unwrap_or("")
                );
                
                api_info.servers.push(Server {
                    url,
                    description: None,
                    variables: std::collections::HashMap::new(),
                });
            }
        }
        
        Ok(api_info)
    }
    
    fn parse_security_definition(
        &self,
        api_info: &mut ParsedApiInfo,
        attribute: &str,
        value: &str,
    ) -> Result<(), ParserError> {
        debug!("Parsing security definition attribute: '{}' with value: '{}'", attribute, value);
        
        let parts: Vec<&str> = attribute.split('.').collect();
        let first_part = parts[0].to_lowercase();
        
        // Handle security scheme property updates (apikey.in, apikey.name, etc.)
        if parts.len() >= 2 {
            let security_type = parts[0].to_lowercase();
            let sub_part = parts.get(1).unwrap().to_string();
            
            // Handle apikey.in, apikey.name, apikey.description
            if security_type == "apikey" && api_info.security_definitions.contains_key(value) {
                if parts.len() >= 2 {
                    let property = sub_part.as_str();
                    if let Some(scheme) = api_info.security_definitions.get_mut(value) {
                        match property {
                            "in" => {
                                scheme.in_type = Some(parts[1].to_string());
                                return Ok(());
                            },
                            "name" => {
                                scheme.name = Some(parts[1].to_string());
                                return Ok(());
                            },
                            "description" => {
                                scheme.description = Some(parts[1].to_string());
                                return Ok(());
                            },
                            _ => {}
                        }
                    }
                }
            }
            // Handle updating existing ApiKeyAuth scheme
            else if security_type == "apikey" && parts.len() >= 2 {
                let property = sub_part.as_str();
                
                if let Some(scheme) = api_info.security_definitions.get_mut("ApiKeyAuth") {
                    match property {
                        "in" => {
                            scheme.in_type = Some(value.to_string());
                            return Ok(());
                        },
                        "name" => {
                            scheme.name = Some(value.to_string());
                            return Ok(());
                        },
                        "description" => {
                            scheme.description = Some(value.to_string());
                            return Ok(());
                        },
                        _ => {}
                    }
                }
            }
            // Handle OAuth2 flow properties like oauth2.implicit.authorizationUrl
            else if security_type == "oauth2" && parts.len() >= 3 {
                let flow_type = sub_part.as_str();
                let property = parts.get(2).unwrap().to_string();
                
                // Check if we have an OAuth2 scheme
                if let Some(scheme) = api_info.security_definitions.get_mut("OAuth2") {
                    if let Some(ref mut flows) = scheme.flows {
                        match property.as_str() {
                            "authorizationUrl" => {
                                if flow_type == "implicit" {
                                    if let Some(ref mut implicit) = flows.implicit {
                                        implicit.authorizationUrl = Some(value.to_string());
                                        return Ok(());
                                    }
                                }
                            },
                            "tokenUrl" => {
                                if flow_type == "password" {
                                    if let Some(ref mut password) = flows.password {
                                        password.tokenUrl = Some(value.to_string());
                                        return Ok(());
                                    }
                                } else if flow_type == "clientcredentials" || flow_type == "application" {
                                    if let Some(ref mut clientCredentials) = flows.clientCredentials {
                                        clientCredentials.tokenUrl = Some(value.to_string());
                                        return Ok(());
                                    }
                                } else if flow_type == "authorizationcode" || flow_type == "accesscode" {
                                    if let Some(ref mut authorizationCode) = flows.authorizationCode {
                                        authorizationCode.tokenUrl = Some(value.to_string());
                                        return Ok(());
                                    }
                                }
                            },
                            "refreshUrl" => {
                                if flow_type == "implicit" {
                                    if let Some(ref mut implicit) = flows.implicit {
                                        implicit.refreshUrl = Some(value.to_string());
                                        return Ok(());
                                    }
                                } else if flow_type == "password" {
                                    if let Some(ref mut password) = flows.password {
                                        password.refreshUrl = Some(value.to_string());
                                        return Ok(());
                                    }
                                } else if flow_type == "clientcredentials" || flow_type == "application" {
                                    if let Some(ref mut clientCredentials) = flows.clientCredentials {
                                        clientCredentials.refreshUrl = Some(value.to_string());
                                        return Ok(());
                                    } 
                                } else if flow_type == "authorizationcode" || flow_type == "accesscode" {
                                    if let Some(ref mut authorizationCode) = flows.authorizationCode {
                                        authorizationCode.refreshUrl = Some(value.to_string());
                                        return Ok(());
                                    }
                                }
                            },
                            "scopes" => {
                                if parts.len() >= 4 {
                                    let scope_name = parts[3].to_string();
                                    
                                    if flow_type == "implicit" {
                                        if let Some(ref mut implicit) = flows.implicit {
                                            implicit.scopes.insert(scope_name, value.to_string());
                                            return Ok(());
                                        }
                                    } else if flow_type == "password" {
                                        if let Some(ref mut password) = flows.password {
                                            password.scopes.insert(scope_name, value.to_string());
                                            return Ok(());
                                        }
                                    } else if flow_type == "clientcredentials" || flow_type == "application" {
                                        if let Some(ref mut clientCredentials) = flows.clientCredentials {
                                            clientCredentials.scopes.insert(scope_name, value.to_string());
                                            return Ok(());
                                        }
                                    } else if flow_type == "authorizationcode" || flow_type == "accesscode" {
                                        if let Some(ref mut authorizationCode) = flows.authorizationCode {
                                            authorizationCode.scopes.insert(scope_name, value.to_string());
                                            return Ok(());
                                        }
                                    }
                                }
                            },
                            _ => {}
                        }
                    }
                }
                // If the OAuth2 scheme doesn't exist yet, try original approach with value as scheme name
                else if let Some(scheme) = api_info.security_definitions.get_mut(value) {
                    if let Some(ref mut flows) = scheme.flows {
                        match property.as_str() {
                            "authorizationUrl" => {
                                if flow_type == "implicit" {
                                    if let Some(ref mut implicit) = flows.implicit {
                                        implicit.authorizationUrl = Some(value.to_string());
                                        return Ok(());
                                    }
                                }
                            },
                            "tokenUrl" => {
                                if flow_type == "password" {
                                    if let Some(ref mut password) = flows.password {
                                        password.tokenUrl = Some(value.to_string());
                                        return Ok(());
                                    }
                                } else if flow_type == "clientcredentials" || flow_type == "application" {
                                    if let Some(ref mut clientCredentials) = flows.clientCredentials {
                                        clientCredentials.tokenUrl = Some(value.to_string());
                                        return Ok(());
                                    }
                                } else if flow_type == "authorizationcode" || flow_type == "accesscode" {
                                    if let Some(ref mut authorizationCode) = flows.authorizationCode {
                                        authorizationCode.tokenUrl = Some(value.to_string());
                                        return Ok(());
                                    }
                                }
                            },
                            "refreshUrl" => {
                                if flow_type == "implicit" {
                                    if let Some(ref mut implicit) = flows.implicit {
                                        implicit.refreshUrl = Some(value.to_string());
                                        return Ok(());
                                    }
                                } else if flow_type == "password" {
                                    if let Some(ref mut password) = flows.password {
                                        password.refreshUrl = Some(value.to_string());
                                        return Ok(());
                                    }
                                } else if flow_type == "clientcredentials" || flow_type == "application" {
                                    if let Some(ref mut clientCredentials) = flows.clientCredentials {
                                        clientCredentials.refreshUrl = Some(value.to_string());
                                        return Ok(());
                                    } 
                                } else if flow_type == "authorizationcode" || flow_type == "accesscode" {
                                    if let Some(ref mut authorizationCode) = flows.authorizationCode {
                                        authorizationCode.refreshUrl = Some(value.to_string());
                                        return Ok(());
                                    }
                                }
                            },
                            "scopes" => {
                                if parts.len() >= 4 {
                                    let scope_name = parts[3].to_string();
                                    
                                    if flow_type == "implicit" {
                                        if let Some(ref mut implicit) = flows.implicit {
                                            implicit.scopes.insert(scope_name, value.to_string());
                                            return Ok(());
                                        }
                                    } else if flow_type == "password" {
                                        if let Some(ref mut password) = flows.password {
                                            password.scopes.insert(scope_name, value.to_string());
                                            return Ok(());
                                        }
                                    } else if flow_type == "clientcredentials" || flow_type == "application" {
                                        if let Some(ref mut clientCredentials) = flows.clientCredentials {
                                            clientCredentials.scopes.insert(scope_name, value.to_string());
                                            return Ok(());
                                        }
                                    } else if flow_type == "authorizationcode" || flow_type == "accesscode" {
                                        if let Some(ref mut authorizationCode) = flows.authorizationCode {
                                            authorizationCode.scopes.insert(scope_name, value.to_string());
                                            return Ok(());
                                        }
                                    }
                                }
                            },
                            _ => {}
                        }
                    }
                }
            }
        }
        
        // Handle direct security type definitions (@securityDefinitions.apikey)
        match first_part.as_str() {
            "apikey" => {
                // apikey requires additional parameters: in, name
                // These are handled separately in subsequent annotations
                api_info.security_definitions.insert(
                    value.to_string(),
                    SecurityScheme {
                        type_: "apiKey".to_string(),
                        description: None,
                        name: None,
                        in_type: None,
                        scheme: None,
                        bearerFormat: None,
                        flows: None,
                        openIdConnectUrl: None,
                    },
                );
                return Ok(());
            },
            "basic" => {
                api_info.security_definitions.insert(
                    value.to_string(),
                    SecurityScheme {
                        type_: "http".to_string(),
                        scheme: Some("basic".to_string()),
                        description: None,
                        name: None,
                        in_type: None,
                        bearerFormat: None,
                        flows: None,
                        openIdConnectUrl: None,
                    },
                );
                return Ok(());
            },
            "bearer" | "jwt" => {
                api_info.security_definitions.insert(
                    value.to_string(),
                    SecurityScheme {
                        type_: "http".to_string(),
                        scheme: Some("bearer".to_string()),
                        bearerFormat: if first_part == "jwt" { Some("JWT".to_string()) } else { None },
                        description: None,
                        name: None,
                        in_type: None,
                        flows: None,
                        openIdConnectUrl: None,
                    },
                );
                return Ok(());
            },
            "oauth2" => {
                if parts.len() >= 2 {
                    let flow_type = parts[1];
                    let mut oauth_flows = OAuthFlows::default();
                    
                    // Create different flows based on the type
                    match flow_type {
                        "implicit" => {
                            oauth_flows.implicit = Some(crate::models::OAuthFlow {
                                scopes: std::collections::HashMap::new(),
                                ..Default::default()
                            });
                        },
                        "password" => {
                            oauth_flows.password = Some(crate::models::OAuthFlow {
                                scopes: std::collections::HashMap::new(),
                                ..Default::default()
                            });
                        },
                        "clientcredentials" | "application" => {
                            oauth_flows.clientCredentials = Some(crate::models::OAuthFlow {
                                scopes: std::collections::HashMap::new(),
                                ..Default::default()
                            });
                        },
                        "authorizationcode" | "accesscode" => {
                            oauth_flows.authorizationCode = Some(crate::models::OAuthFlow {
                                scopes: std::collections::HashMap::new(),
                                ..Default::default()
                            });
                        },
                        _ => {
                            return Err(ParserError::SecurityParseError(
                                format!("Unknown OAuth2 flow type: {}", flow_type)
                            ));
                        }
                    }
                    
                    api_info.security_definitions.insert(
                        value.to_string(),
                        SecurityScheme {
                            type_: "oauth2".to_string(),
                            description: None,
                            name: None,
                            in_type: None,
                            scheme: None,
                            bearerFormat: None,
                            flows: Some(oauth_flows),
                            openIdConnectUrl: None,
                        },
                    );
                    return Ok(());
                } else {
                    return Err(ParserError::SecurityParseError(
                        format!("OAuth2 requires a flow type: {}", attribute)
                    ));
                }
            },
            "openidconnect" => {
                // OpenAPI 3.0+ OpenID Connect
                api_info.security_definitions.insert(
                    value.to_string(),
                    SecurityScheme {
                        type_: "openIdConnect".to_string(),
                        openIdConnectUrl: Some(parts.get(1).unwrap_or(&"").to_string()),
                        description: None,
                        name: None,
                        in_type: None,
                        scheme: None,
                        bearerFormat: None,
                        flows: None,
                    },
                );
                return Ok(());
            },
            _ => {}
        }
        
        // If we get here and parts.len() < 2, it's an error
        if parts.len() < 2 {
            return Err(ParserError::SecurityParseError(
                format!("Invalid security definition attribute: {}", attribute)
            ));
        }
        
        Ok(())
    }
    
    #[allow(non_snake_case)]
    pub fn parse_operations(&self, directories: &[impl AsRef<Path>], excluded_dirs: &[impl AsRef<Path>], base_dir: impl AsRef<Path>) -> Result<(Vec<ParsedOperation>, HashMap<String, Schema>)> {
        let mut operations = Vec::new();
        let mut all_file_paths = Vec::new();
        
        // First, collect all .go files in the specified directories recursively
        for dir in directories {
            // Make sure we're handling relative paths correctly
            let dir_path = if dir.as_ref().is_absolute() {
                dir.as_ref().to_path_buf()
            } else {
                PathBuf::from(dir.as_ref())
            };
            
            debug!("Scanning for Go files in directory: {}", dir_path.display());
            
            self.collect_go_files_recursively(&dir_path, excluded_dirs, &mut all_file_paths);
        }
        
        debug!("Found {} Go files to parse", all_file_paths.len());
        
        // Extract examples from structs with import resolution
        let struct_examples = self.extract_struct_examples_with_imports(&all_file_paths, base_dir.as_ref());
        debug!("Extracted examples from {} structs (including imported models)", struct_examples.len());
        
        // Collect all model references from operations we find
        let mut referenced_models = HashSet::new();
        
        // Now parse all files for operations
        for file_path in &all_file_paths {
            if let Ok(content) = std::fs::read_to_string(file_path) {
                let mut current_annotations: Vec<Annotation> = Vec::new();
                
                for line in content.lines() {
                    if let Some(captures) = ANNOTATION_REGEX.captures(line) {
                        let annotation_type_str = captures.get(1).unwrap().as_str();
                        let attribute = captures.get(2).map(|m| m.as_str().to_string());
                        let value = captures.get(3).unwrap().as_str().to_string();
                        
                        let annotation = Annotation {
                            annotation_type: AnnotationType::from(annotation_type_str),
                            attribute,
                            value,
                        };
                        
                        current_annotations.push(annotation);
                    } else if line.trim().starts_with("func ") && !current_annotations.is_empty() {
                        // Function encountered, process the collected annotations
                        let router_annotation = current_annotations.iter().find(|a| {
                            matches!(a.annotation_type, AnnotationType::Router | AnnotationType::DeprecatedRouter)
                        });
                        
                        if router_annotation.is_some() {
                            match self.parse_operation_with_examples(&current_annotations, &struct_examples) {
                                Ok(operation) => {
                                    // Collect schemas from this operation
                                    self.collect_operation_schema_refs(&operation, &mut referenced_models);
                                    operations.push(operation);
                                },
                                Err(e) => warn!("Failed to parse operation: {}", e),
                            }
                        }
                        
                        current_annotations.clear();
                    }
                }
            }
        }
        
        // Add common response types
        referenced_models.insert("response.ApiResponse".to_string());
        referenced_models.insert("response.Response".to_string());
        referenced_models.insert("response.OpenApiResponse".to_string());
        referenced_models.insert("response.OpenApiErrorNonSnap".to_string());
        
        debug!("Found {} referenced models", referenced_models.len());
        
        // Extract schemas for the referenced models
        let struct_schemas = self.extract_referenced_schemas(&all_file_paths, &referenced_models);
        debug!("Extracted schemas for {} referenced models", struct_schemas.len());
        
        Ok((operations, struct_schemas))
    }
    
    // Extract schema references from an operation
    fn collect_operation_schema_refs(&self, operation: &ParsedOperation, referenced_models: &mut HashSet<String>) {
        // Check parameters
        for param in &operation.operation.parameters {
            if let Some(schema) = &param.schema {
                self.collect_schema_references(schema, referenced_models);
            }
        }
        
        // Check request body
        if let Some(req_body) = &operation.operation.requestBody {
            for (_, media_type) in &req_body.content {
                if let Some(schema) = &media_type.schema {
                    self.collect_schema_references(schema, referenced_models);
                }
            }
        }
        
        // Check responses
        for (_, response) in &operation.operation.responses {
            for (_, media_type) in &response.content {
                if let Some(schema) = &media_type.schema {
                    self.collect_schema_references(schema, referenced_models);
                }
            }
        }
        
        // Extract generic parameterized types like ApiResponse{data=User}
        let mut generics_to_process = Vec::new();
        
        for model in referenced_models.iter() {
            if model.contains('{') && model.contains('}') {
                generics_to_process.push(model.clone());
            }
        }
        
        for generic_ref in &generics_to_process {
            // Handle patterns like "response.ApiResponse{data=userModel.User}"
            if generic_ref.contains("=") {
                let parts: Vec<&str> = generic_ref.split('=').collect();
                if parts.len() >= 2 {
                    let inner_type = parts[1].trim().trim_end_matches('}');
                    referenced_models.insert(inner_type.to_string());
                }
            }
        }
    }
    
    // Recursively collect schema references
    fn collect_schema_references(&self, schema: &Schema, references: &mut HashSet<String>) {
        // Check for direct reference
        if let Some(ref_) = &schema.ref_ {
            // Extract the model name from the reference
            let parts: Vec<&str> = ref_.split('/').collect();
            if parts.len() >= 4 && parts[1] == "components" && parts[2] == "schemas" {
                references.insert(parts[3].to_string());
            } else if !ref_.starts_with("/") && !ref_.starts_with("#/") {
                // It's a direct model reference
                references.insert(ref_.to_string());
            }
        }
        
        // Check array items
        if let Some(items) = &schema.items {
            self.collect_schema_references(items, references);
        }
        
        // Check properties
        for (_, property) in &schema.properties {
            self.collect_schema_references(property, references);
        }
        
        // Check additionalProperties
        if let Some(add_props) = &schema.additionalProperties {
            if let Ok(schema_value) = serde_json::from_value::<Schema>(add_props.clone()) {
                self.collect_schema_references(&schema_value, references);
            }
        }
        
        // Check composition schemas
        if let Some(all_of) = &schema.allOf {
            for s in all_of {
                self.collect_schema_references(s, references);
            }
        }
        
        if let Some(any_of) = &schema.anyOf {
            for s in any_of {
                self.collect_schema_references(s, references);
            }
        }
        
        if let Some(one_of) = &schema.oneOf {
            for s in one_of {
                self.collect_schema_references(s, references);
            }
        }
        
        if let Some(not) = &schema.not {
            self.collect_schema_references(not, references);
        }
    }
    
    // Original collect_referenced_models method renamed to avoid conflicts
    #[allow(dead_code)]
    fn collect_model_references_from_annotations(&self, file_paths: &[PathBuf]) -> HashSet<String> {
        // Original implementation...
        let mut referenced_models = HashSet::new();
        
        // Patterns to match model references in annotations
        let param_regex = Regex::new(r"@Param\s+\w+\s+body\s+([a-zA-Z0-9_.]+)").unwrap();
        let request_body_regex = Regex::new(r"@RequestBody\s+.*\{object\}\s+([a-zA-Z0-9_.]+)").unwrap();
        // Make the response regex more flexible to handle various whitespace patterns
        let response_regex = Regex::new(r"@(?:Success|Failure|Response)\s+\d+\s+\{(?:object|array)\}\s*([a-zA-Z0-9_.]+)").unwrap();
        
        for file_path in file_paths {
            if let Ok(content) = std::fs::read_to_string(file_path) {
                // Find all model references in @Param annotations
                for cap in param_regex.captures_iter(&content) {
                    if let Some(m) = cap.get(1) {
                        let model_name = m.as_str().to_string();
                        referenced_models.insert(model_name.clone());
                        
                        // Also add the bare model name without package prefix
                        if model_name.contains('.') {
                            if let Some(base_name) = model_name.split('.').last() {
                                referenced_models.insert(base_name.to_string());
                            }
                        }
                    }
                }
                
                // Find all model references in @RequestBody annotations
                for cap in request_body_regex.captures_iter(&content) {
                    if let Some(m) = cap.get(1) {
                        let model_name = m.as_str().to_string();
                        referenced_models.insert(model_name.clone());
                        
                        // Also add the bare model name without package prefix
                        if model_name.contains('.') {
                            if let Some(base_name) = model_name.split('.').last() {
                                referenced_models.insert(base_name.to_string());
                            }
                        }
                    }
                }
                
                // Find all model references in @Success, @Failure, and @Response annotations
                for cap in response_regex.captures_iter(&content) {
                    if let Some(m) = cap.get(1) {
                        let model_name = m.as_str().to_string();
                        referenced_models.insert(model_name.clone());
                        
                        // Also add the bare model name without package prefix
                        if model_name.contains('.') {
                            if let Some(base_name) = model_name.split('.').last() {
                                referenced_models.insert(base_name.to_string());
                            }
                        }
                    }
                }
            }
        }
        
        debug!("Found referenced models: {:?}", referenced_models);
        referenced_models
    }
    
    // Helper method to recursively collect Go files in a directory and its subdirectories
    fn collect_go_files_recursively(&self, dir_path: &Path, excluded_dirs: &[impl AsRef<Path>], file_paths: &mut Vec<PathBuf>) {
        use walkdir::WalkDir;
        
        // Convert excluded directories to absolute paths for easier comparison
        let excluded_paths: Vec<PathBuf> = excluded_dirs
            .iter()
            .map(|p| {
                if p.as_ref().is_absolute() {
                    p.as_ref().to_path_buf()
                } else {
                    std::env::current_dir().unwrap_or_default().join(p.as_ref())
                }
            })
            .collect();
        
        for entry in WalkDir::new(dir_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| !e.file_type().is_dir())
        {
            let path = entry.path();
            
            // Skip if the path is in an excluded directory
            let should_skip = excluded_paths.iter().any(|excluded| {
                path.starts_with(excluded) || 
                // Also check relative paths
                path.components().any(|component| {
                    if let std::path::Component::Normal(comp) = component {
                        excluded_paths.iter().any(|ex| ex.ends_with(comp))
                    } else {
                        false
                    }
                })
            });
            
            if should_skip {
                debug!("Skipping excluded file: {:?}", path);
                continue;
            }
            
            if path.extension().map_or(false, |ext| ext == "go") {
                debug!("Found Go file: {:?}", path);
                file_paths.push(path.to_path_buf());
            }
        }
    }
    
    fn parse_operation_with_examples(
        &self, 
        annotations: &[Annotation], 
        struct_examples: &HashMap<String, HashMap<String, serde_json::Value>>
    ) -> Result<ParsedOperation, ParserError> {
        let mut operation = Operation::default();
        let mut path = String::new();
        let mut method = String::new();
        let mut request_body_schema_ref: Option<String> = None;
        
        for annotation in annotations {
            match &annotation.annotation_type {
                AnnotationType::Id => {
                    operation.operationId = Some(annotation.value.clone());
                },
                AnnotationType::Summary => {
                    operation.summary = Some(annotation.value.clone());
                },
                AnnotationType::Description => {
                    operation.description = Some(annotation.value.clone());
                },
                AnnotationType::Tags => {
                    annotation.value.split(',').map(|s| s.trim()).for_each(|tag| {
                        operation.tags.push(tag.to_string());
                    });
                },
                AnnotationType::Router | AnnotationType::DeprecatedRouter => {
                    if let Some(captures) = ROUTER_REGEX.captures(&annotation.value) {
                        path = format!("/{}", captures.get(1).unwrap().as_str());
                        method = captures.get(2).unwrap().as_str().to_lowercase();
                        
                        // Mark as deprecated if using deprecated router
                        if let AnnotationType::DeprecatedRouter = annotation.annotation_type {
                            operation.deprecated = Some(true);
                        }
                    } else {
                        return Err(ParserError::RouterParseError(
                            format!("Invalid router format: {}", annotation.value)
                        ));
                    }
                },
                AnnotationType::Accept => {
                    // Add request body with media type
                    annotation.value.split(',').map(|s| s.trim()).for_each(|media_type| {
                        let normalized_type = self.normalize_mime_type(media_type);
                        operation.consumes.push(normalized_type.clone());
                        
                        if operation.requestBody.is_none() {
                            operation.requestBody = Some(RequestBody {
                                description: None,
                                content: HashMap::new(),
                                required: Some(true),
                            });
                        }
                        
                        if let Some(ref mut request_body) = operation.requestBody {
                            request_body.content.insert(normalized_type, MediaType::default());
                        }
                    });
                },
                AnnotationType::Produce => {
                    // Store media types for later use in responses
                    annotation.value.split(',').map(|s| s.trim()).for_each(|media_type| {
                        let normalized_type = self.normalize_mime_type(media_type);
                        operation.produces.push(normalized_type);
                    });
                },
                AnnotationType::Param => {
                    match self.parse_parameter(&annotation.value) {
                        Ok(parameter) => {
                            // For body parameters, extract schema ref for request body
                            if parameter.in_type == "body" {
                                if parameter.schema.is_some() {
                                    if let Some(ref schema) = parameter.schema {
                                        if let Some(ref_) = &schema.ref_ {
                                            // Extract model name for later use with examples
                                            let model_name = ref_.split('/').last().unwrap_or("").to_string();
                                            request_body_schema_ref = Some(model_name);
                                        } else if let Some(type_value) = &schema.type_ {
                                            // If it's a direct type reference
                                            if let Some(type_str) = type_value.as_str() {
                                                request_body_schema_ref = Some(type_str.to_string());
                                            }
                                        }
                                    }
                                } else {
                                    // Try to use the parameter name as a model name (common pattern)
                                    request_body_schema_ref = Some(parameter.name.clone());
                                }

                                // Ensure we have a requestBody
                                if operation.requestBody.is_none() {
                                    operation.requestBody = Some(RequestBody {
                                        description: parameter.description.clone(),
                                        content: HashMap::new(),
                                        required: parameter.required,
                                    });
                                }
                                
                                // Add schema and content type for the request body
                                if let Some(ref mut request_body) = operation.requestBody {
                                    if let Some(schema) = parameter.schema.clone() {
                                        let schema_ref = schema.ref_.clone();
                                        
                                        // Add to all content types or create application/json if none
                                        if request_body.content.is_empty() {
                                            let mut media_type = MediaType::default();
                                            media_type.schema = Some(schema);
                                            request_body.content.insert("application/json".to_string(), media_type);
                                        } else {
                                            for (_content_type, media_type) in request_body.content.iter_mut() {
                                                media_type.schema = Some(schema.clone());
                                            }
                                        }
                                        
                                        // If we have a schema reference, check if we can find examples for it
                                        if let Some(ref_str) = schema_ref {
                                            // Extract the model name from the reference
                                            let model_name = ref_str.split('/').last().unwrap_or("").to_string();
                                            debug!("Looking for examples for model: {}", model_name);
                                            
                                            // Try different variations of the model name to find examples
                                            let possible_model_names = vec![
                                                model_name.clone(),
                                                // For models with package prefix like userModel.UserLoginRequest
                                                if model_name.contains('.') {
                                                    model_name.split('.').last().unwrap_or("").to_string()
                                                } else {
                                                    model_name.clone()
                                                }
                                            ];
                                            
                                            for possible_name in possible_model_names {
                                                if let Some(examples) = struct_examples.get(&possible_name) {
                                                    debug!("Found examples for model: {}", possible_name);
                                                    let example_value = serde_json::to_value(examples).unwrap_or(serde_json::Value::Null);
                                                    
                                                    if !example_value.is_null() {
                                                        for (_content_type, media_type) in request_body.content.iter_mut() {
                                                            media_type.example = Some(example_value.clone());
                                                        }
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            
                            // Add parameter to the operation (except 'body' parameters in OpenAPI 3)
                            if parameter.in_type != "body" {
                                operation.parameters.push(parameter);
                            }
                        },
                        Err(e) => {
                            warn!("Failed to parse parameter: {}", e);
                        }
                    }
                },
                AnnotationType::RequestBody => {
                    // Direct request body annotation handling
                    if operation.requestBody.is_none() {
                        operation.requestBody = Some(RequestBody {
                            description: Some(annotation.value.clone()),
                            content: HashMap::new(),
                            required: Some(true),
                        });
                    }
                    
                    // Check if the annotation value contains a model reference
                    if annotation.value.contains("{") && annotation.value.contains("}") {
                        let start = annotation.value.find("{").unwrap();
                        let end = annotation.value.find("}").unwrap();
                        let model_type = &annotation.value[start+1..end];
                        
                        // Parse model type, looking for "object ModelName"
                        let parts: Vec<&str> = model_type.split_whitespace().collect();
                        if parts.len() >= 2 && parts[0] == "object" {
                            let model_name = parts[1];
                            request_body_schema_ref = Some(model_name.to_string());
                        } else if !parts.is_empty() {
                            // Model name might be directly specified
                            request_body_schema_ref = Some(parts[0].to_string());
                        }
                    }
                },
                AnnotationType::Response => {
                    match self.parse_response(&annotation.value) {
                        Ok(mut response) => {
                            // If response has no content but we have produces, add them
                            if response.content.is_empty() && !operation.produces.is_empty() {
                                for content_type in &operation.produces {
                                    response.content.insert(content_type.clone(), MediaType::default());
                                }
                            }
                            
                            // Set examples for each response media type
                            for (_content_type, media_type) in &mut response.content {
                                if let Some(schema) = &media_type.schema {
                                    // Try to find examples if we have a schema reference
                                if let Some(ref_) = &schema.ref_ {
                                        // Extract the model name from the reference
                                        let full_type_name = ref_.split('/').last().unwrap_or("");
                                        debug!("Looking for response examples for model: {}", full_type_name);
                                        
                                        // Try different variations of the model name to find examples
                                        let possible_names = vec![
                                            full_type_name.to_string(),
                                            if full_type_name.contains('.') {
                                                full_type_name.split('.').last().unwrap_or("").to_string()
                                            } else {
                                                full_type_name.to_string()
                                            }
                                        ];
                                        
                                        for name in possible_names {
                                            debug!("Checking for examples with name: {}", name);
                                            if let Some(examples) = struct_examples.get(&name) {
                                                debug!("Found examples for response model: {}", name);
                                                let example_value = serde_json::to_value(examples)
                                                    .unwrap_or(serde_json::Value::Null);
                                                
                                                if !example_value.is_null() && media_type.example.is_none() {
                                                    debug!("Setting example for response: {:?}", example_value);
                                                media_type.example = Some(example_value);
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    // Handle array items
                                    else if let Some(items) = &schema.items {
                                        if let Some(ref_) = &items.ref_ {
                                            let item_type_name = ref_.split('/').last().unwrap_or("");
                                            debug!("Looking for response array item examples for model: {}", item_type_name);
                                            
                                            let possible_names = vec![
                                                item_type_name.to_string(),
                                                if item_type_name.contains('.') {
                                                    item_type_name.split('.').last().unwrap_or("").to_string()
                                                } else {
                                                    item_type_name.to_string()
                                                }
                                            ];
                                            
                                            for name in possible_names {
                                                if let Some(examples) = struct_examples.get(&name) {
                                                    debug!("Found examples for array item model: {}", name);
                                                    let item_example = serde_json::to_value(examples)
                                                        .unwrap_or(serde_json::Value::Null);
                                                    
                                                    if !item_example.is_null() && media_type.example.is_none() {
                                                        // Create an array example
                                                        let array_example = serde_json::Value::Array(vec![item_example]);
                                                        media_type.example = Some(array_example);
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            
                            operation.responses.insert(response.code.clone(), response);
                        },
                        Err(e) => {
                            warn!("Failed to parse response: {}", e);
                        }
                    }
                },
                AnnotationType::Security => {
                    let mut security_requirement = HashMap::new();
                    let parts: Vec<&str> = annotation.value.split_whitespace().collect();
                    
                    if !parts.is_empty() {
                        let security_name = parts[0];
                        let scopes: Vec<String> = parts.iter().skip(1).map(|s| s.to_string()).collect();
                        security_requirement.insert(security_name.to_string(), scopes);
                        operation.security.push(security_requirement);
                    }
                },
                _ => {
                    // Other annotations are not directly relevant to operations
                }
            }
        }
        
        // Add examples to request body if we have a schema reference
        if let Some(model_name) = request_body_schema_ref {
            // Try to find examples for this model or split the model name if it has a package prefix
            let possible_model_names = vec![
                model_name.clone(),
                if model_name.contains('.') {
                    model_name.split('.').last().unwrap_or("").to_string()
                } else {
                    model_name.clone()
                }
            ];
            
            for possible_name in possible_model_names {
                if let Some(examples) = struct_examples.get(&possible_name) {
                let example_value = serde_json::to_value(examples).unwrap_or(serde_json::Value::Null);
                if !example_value.is_null() {
                    // Ensure we have a request body
                    if operation.requestBody.is_none() {
                        operation.requestBody = Some(RequestBody {
                            description: None,
                            content: HashMap::new(),
                            required: Some(true),
                        });
                    }
                    
                    // Add example to all content types or add application/json if none
                    if let Some(ref mut request_body) = operation.requestBody {
                        if request_body.content.is_empty() {
                            let mut media_type = MediaType::default();
                            media_type.example = Some(example_value.clone());
                            media_type.schema = Some(Schema {
                                ref_: Some(format!("#/components/schemas/{}", model_name)),
                                ..Default::default()
                            });
                            request_body.content.insert("application/json".to_string(), media_type);
                        } else {
                            for (_content_type, media_type) in request_body.content.iter_mut() {
                                media_type.example = Some(example_value.clone());
                                if media_type.schema.is_none() {
                                    media_type.schema = Some(Schema {
                                        ref_: Some(format!("#/components/schemas/{}", model_name)),
                                        ..Default::default()
                                    });
                                }
                            }
                        }
                        }
                        break;
                    }
                }
            }
        }
        
        // Generate operation ID if not provided
        if operation.operationId.is_none() {
            let operation_id = match method.as_str() {
                "get" => format!("get{}", path.replace("/", "_")),
                "post" => format!("post{}", path.replace("/", "_")),
                "put" => format!("put{}", path.replace("/", "_")),
                "delete" => format!("delete{}", path.replace("/", "_")),
                "patch" => format!("patch{}", path.replace("/", "_")),
                "head" => format!("head{}", path.replace("/", "_")),
                "options" => format!("options{}", path.replace("/", "_")),
                _ => format!("{}_{}", method, path.replace("/", "_")),
            };
            operation.operationId = Some(operation_id);
        }
        
        // If we have no produces but have responses, add a default content type
        if operation.produces.is_empty() && !operation.responses.is_empty() {
            operation.produces.push("application/json".to_string());
            
            // Add the content type to all responses that don't have content
            for (_, response) in operation.responses.iter_mut() {
                if response.content.is_empty() {
                    response.content.insert("application/json".to_string(), MediaType::default());
                }
            }
        }
        
        Ok(ParsedOperation {
            path,
            method,
            operation,
        })
    }
    
    // Add a new method to extract imports from a file
    fn extract_imports(&self, file_content: &str) -> Vec<ImportInfo> {
        let mut imports = Vec::new();
        let import_regex = Regex::new(r#"import\s+\(\s*((?:[^()]*\n)+)\s*\)"#).unwrap();
        let single_import_regex = Regex::new(r#"import\s+(?:([a-zA-Z0-9_]+)\s+)?"([^"]+)""#).unwrap();
        let alias_import_regex = Regex::new(r#"\s*(?:([a-zA-Z0-9_]+)\s+)?"([^"]+)""#).unwrap();
        
        if let Some(caps) = import_regex.captures(file_content) {
            if let Some(import_block) = caps.get(1) {
                for line in import_block.as_str().lines() {
                    if let Some(m) = alias_import_regex.captures(line) {
                        let alias = m.get(1).map_or_else(
                            || {
                                // If no alias, use the last part of the path
                                let path = m.get(2).unwrap().as_str();
                                path.split('/').last().unwrap_or(path).to_string()
                            },
                            |a| a.as_str().to_string(),
                        );
                        let path = m.get(2).unwrap().as_str().to_string();
                        imports.push(ImportInfo {
                            alias,
                            path,
                            file_path: None,
                        });
                    }
                }
            }
        } else {
            // Check for single line imports
            for caps in single_import_regex.captures_iter(file_content) {
                let alias = caps.get(1).map_or_else(
                    || {
                        // If no alias, use the last part of the path
                        let path = caps.get(2).unwrap().as_str();
                        path.split('/').last().unwrap_or(path).to_string()
                    },
                    |a| a.as_str().to_string(),
                );
                let path = caps.get(2).unwrap().as_str().to_string();
                imports.push(ImportInfo {
                    alias,
                    path,
                    file_path: None,
                });
            }
        }
        
        debug!("Extracted {} imports", imports.len());
        imports
    }

    // Add a method to resolve import paths to actual files
    fn resolve_import_paths(&self, 
                           imports: &mut Vec<ImportInfo>, 
                           base_dir: &Path, 
                           go_mod_path: Option<&Path>) -> Result<()> {
        let go_path = std::env::var("GOPATH").unwrap_or_else(|_| "/tmp".to_string());
        let go_modules_cache = PathBuf::from(go_path.clone()).join("pkg/mod");
        
        // First try to find go.mod file if not provided
        let go_mod_file = if let Some(path) = go_mod_path {
            Some(path.to_path_buf())
        } else {
            let mut current_dir = base_dir.to_path_buf();
            let mut found_go_mod = None;
            
            while current_dir.parent().is_some() {
                let go_mod = current_dir.join("go.mod");
                if go_mod.exists() {
                    debug!("Found go.mod file at: {:?}", go_mod);
                    found_go_mod = Some(go_mod);
                    break;
                }
                current_dir = current_dir.parent().unwrap().to_path_buf();
            }
            
            found_go_mod
        };
        
        // Parse go.mod file to get module name
        let module_name = if let Some(path) = &go_mod_file {
            if let Ok(content) = std::fs::read_to_string(path) {
                let module_regex = Regex::new(r#"module\s+([^\s]+)"#).unwrap();
                module_regex.captures(&content)
                    .and_then(|caps| caps.get(1))
                    .map(|m| m.as_str().to_string())
            } else {
                None
            }
        } else {
            None
        };
        
        debug!("Go module name: {:?}", module_name);
        
        for import in imports.iter_mut() {
            // Handle internal (relative to module) imports
            if let Some(module) = &module_name {
                if import.path.starts_with(module) {
                    // Convert module-relative path to filesystem path
                    let rel_path = import.path.strip_prefix(module).unwrap_or(&import.path);
                    let rel_path = rel_path.trim_start_matches('/');
                    
                    // Find go.mod directory
                    let go_mod_dir = go_mod_file.as_ref().and_then(|p| p.parent()).unwrap_or(base_dir);
                    let potential_path = go_mod_dir.join(rel_path);
                    
                    if potential_path.exists() {
                        debug!("Resolved internal import: {} to {:?}", import.path, potential_path);
                        import.file_path = Some(potential_path);
                        continue;
                    }
                }
            }
            
            // Handle standard library imports
            if !import.path.contains('/') {
                // Standard library, no need to resolve
                continue;
            }
            
            // Check GOROOT (standard library location)
            let go_root = std::env::var("GOROOT").unwrap_or_else(|_| "".to_string());
            if !go_root.is_empty() {
                let std_lib_path = PathBuf::from(go_root).join("src").join(&import.path);
                if std_lib_path.exists() {
                    debug!("Resolved stdlib import: {} to {:?}", import.path, std_lib_path);
                    import.file_path = Some(std_lib_path);
                    continue;
                }
            }
            
            // Check for third-party modules in Go modules cache
            if go_modules_cache.exists() {
                // Handle versioned imports in the module cache
                let mut potential_paths = Vec::new();
                
                // Check different version directories in the Go module cache
                for entry in std::fs::read_dir(&go_modules_cache).ok().into_iter().flatten() {
                    if let Ok(entry) = entry {
                        let path = entry.path();
                        if path.is_dir() {
                            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                            if dir_name.starts_with(&import.path) {
                                potential_paths.push(path);
                            }
                        }
                    }
                }
                
                // Sort by version (assuming newer versions come later alphabetically)
                potential_paths.sort();
                
                // Use the latest version
                if let Some(latest) = potential_paths.pop() {
                    debug!("Resolved third-party import: {} to {:?}", import.path, latest);
                    import.file_path = Some(latest);
                    continue;
                }
            }
            
            // Last resort: check GOPATH
            let gopath_src = PathBuf::from(&go_path).join("src");
            let gopath_import = gopath_src.join(&import.path);
            if gopath_import.exists() {
                debug!("Resolved GOPATH import: {} to {:?}", import.path, gopath_import);
                import.file_path = Some(gopath_import);
                continue;
            }
            
            debug!("Could not resolve import path: {}", import.path);
        }
        
        Ok(())
    }

    // Add a method to find model files by name, recursively searching through imports
    fn find_model_file(&self, 
                       model_name: &str, 
                       imports: &[ImportInfo], 
                       file_content: &str,
                       visited: &mut HashSet<String>) -> Option<(PathBuf, String)> {
        // First, check if the model is defined in the current file
        let model_regex = Regex::new(&format!(r"type\s+{}\s+struct", model_name)).unwrap();
        if model_regex.is_match(file_content) {
            return None; // Model is in the current file, no need to find an external file
        }
        
        // Look through imports to find the model
        for import in imports {
            // Skip if we've already visited this import to prevent cycles
            if visited.contains(&import.path) {
                continue;
            }
            visited.insert(import.path.clone());
            
            // Check if the model reference includes this import's alias
            let ref_pattern = format!(r"{}\.\s*{}", import.alias, model_name);
            let ref_regex = Regex::new(&ref_pattern).unwrap();
            
            if ref_regex.is_match(file_content) {
                // Found a reference, now check if we can find the model file
                if let Some(file_path) = &import.file_path {
                    // Try to find Go files in this directory
                    if file_path.is_dir() {
                        for entry in std::fs::read_dir(file_path).ok().into_iter().flatten() {
                            if let Ok(entry) = entry {
                                let path = entry.path();
                                if path.extension().map_or(false, |ext| ext == "go") {
                                    if let Ok(content) = std::fs::read_to_string(&path) {
                                        if model_regex.is_match(&content) {
                                            debug!("Found model {} in file {:?}", model_name, path);
                                            return Some((path, content));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        None
    }

    // Enhance the extract_struct_examples method to handle models from imports
    pub fn extract_struct_examples_with_imports(&self, 
                                               file_paths: &[PathBuf], 
                                               base_dir: impl AsRef<Path>) -> HashMap<String, HashMap<String, serde_json::Value>> {
        let mut struct_examples: HashMap<String, HashMap<String, serde_json::Value>> = HashMap::new();
        let imported_models_cache = Arc::new(Mutex::new(HashMap::<String, PathBuf>::new()));
        
        // Regular expressions for struct parsing
        let struct_regex = Regex::new(r"type\s+(\w+)\s+struct\s*\{").unwrap();
        let field_regex = Regex::new(r#"^\s*(\w+)\s+\w+\s+`[^`]*example:"([^"]*)"`"#).unwrap();
        
        for file_path in file_paths {
            if let Ok(content) = std::fs::read_to_string(file_path) {
                // Extract imports from this file
                let mut imports = self.extract_imports(&content);
                let _ = self.resolve_import_paths(&mut imports, base_dir.as_ref(), None);
                
                // First pass: collect all local struct examples
                let mut i = 0;
                let lines: Vec<&str> = content.lines().collect();
                
                while i < lines.len() {
                    if let Some(captures) = struct_regex.captures(lines[i]) {
                        let struct_name = captures.get(1).unwrap().as_str();
                        debug!("Found struct: {}", struct_name);
                        
                        let mut field_examples = HashMap::new();
                        let mut j = i + 1;
                        
                        // Parse fields until we reach the closing brace
                        while j < lines.len() && !lines[j].trim().starts_with("}") {
                            if let Some(field_captures) = field_regex.captures(lines[j]) {
                                let field_name = field_captures.get(1).unwrap().as_str();
                                let example_value = field_captures.get(2).unwrap().as_str();
                                
                                debug!("  Field: {} with example: {}", field_name, example_value);
                                
                                // Try to parse as JSON
                                let json_value = if let Ok(json) = serde_json::from_str::<serde_json::Value>(example_value) {
                                    json
                                } else if let Ok(json) = serde_json::from_str::<serde_json::Value>(&format!("\"{}\"", example_value)) {
                                    json
                                } else {
                                    serde_json::Value::String(example_value.to_string())
                                };
                                
                                field_examples.insert(field_name.to_string(), json_value);
                            }
                            j += 1;
                        }
                        
                        if !field_examples.is_empty() {
                            struct_examples.insert(struct_name.to_string(), field_examples);
                        }
                        
                        i = j;
                    }
                    i += 1;
                }
                
                // Second pass: find models in body parameters and responses that refer to external files
                let param_body_regex = Regex::new(r"@Param\s+\w+\s+body\s+([a-zA-Z0-9_.]+)").unwrap();
                let response_body_regex = Regex::new(r"@(?:Success|Failure)\s+\d+\s+\{object\}\s+([a-zA-Z0-9_.]+)").unwrap();
                
                let mut model_refs = HashSet::new();
                
                // Collect all model references
                for cap in param_body_regex.captures_iter(&content) {
                    if let Some(m) = cap.get(1) {
                        model_refs.insert(m.as_str().to_string());
                    }
                }
                
                for cap in response_body_regex.captures_iter(&content) {
                    if let Some(m) = cap.get(1) {
                        model_refs.insert(m.as_str().to_string());
                    }
                }
                
                // Process each model reference to find it in import files
                for model_ref in model_refs {
                    // Skip if we've already processed this model
                    if struct_examples.contains_key(&model_ref) {
                        continue;
                    }
                    
                    // Check if it's a qualified name (with package)
                    let parts: Vec<&str> = model_ref.split('.').collect();
                    if parts.len() >= 2 {
                        let package_alias = parts[0];
                        let model_name = parts[1];
                        
                        // Find the import that matches this package
                        for import in &imports {
                            if import.alias == package_alias {
                                // Try to find the model in the imported package
                                let mut visited = HashSet::new();
                                if let Some((model_file, model_content)) = self.find_model_file(model_name, &imports, &content, &mut visited) {
                                    // Parse the model file to extract examples
                                    let model_lines: Vec<&str> = model_content.lines().collect();
                                    let model_struct_pattern = format!(r"type\s+{}\s+struct\s*\{{", model_name);
                                    let model_struct_regex = Regex::new(&model_struct_pattern).unwrap();
                                    
                                    let mut i = 0;
                                    while i < model_lines.len() {
                                        if let Some(_) = model_struct_regex.captures(model_lines[i]) {
                                            let mut field_examples = HashMap::new();
                                            let mut j = i + 1;
                                            
                                            // Parse fields until we reach the closing brace
                                            while j < model_lines.len() && !model_lines[j].trim().starts_with("}") {
                                                if let Some(field_captures) = field_regex.captures(model_lines[j]) {
                                                    let field_name = field_captures.get(1).unwrap().as_str();
                                                    let example_value = field_captures.get(2).unwrap().as_str();
                                                    
                                                    debug!("  Field in imported model: {} with example: {}", field_name, example_value);
                                                    
                                                    // Try to parse as JSON
                                                    let json_value = if let Ok(json) = serde_json::from_str::<serde_json::Value>(example_value) {
                                                        json
                                                    } else if let Ok(json) = serde_json::from_str::<serde_json::Value>(&format!("\"{}\"", example_value)) {
                                                        json
                                                    } else {
                                                        serde_json::Value::String(example_value.to_string())
                                                    };
                                                    
                                                    field_examples.insert(field_name.to_string(), json_value);
                                                }
                                                j += 1;
                                            }
                                            
                                            if !field_examples.is_empty() {
                                                // Store with the full qualified name
                                                struct_examples.insert(model_ref.clone(), field_examples.clone());
                                                // Also store with just the model name for backwards compatibility
                                                struct_examples.insert(model_name.to_string(), field_examples);
                                            }
                                            
                                            break;
                                        }
                                        i += 1;
                                    }
                                    
                                    // Cache this model file for future reference
                                    imported_models_cache.lock().unwrap().insert(model_ref.clone(), model_file);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        
        struct_examples
    }

    // Add back the normalize_mime_type method that was removed
    fn normalize_mime_type(&self, mime_type: &str) -> String {
        match mime_type.to_lowercase().as_str() {
            "json" => "application/json".to_string(),
            "xml" => "application/xml".to_string(),
            "plain" | "text" => "text/plain".to_string(),
            "html" => "text/html".to_string(),
            "form" | "form-data" | "multipart" => "multipart/form-data".to_string(),
            "form-urlencoded" | "urlencoded" => "application/x-www-form-urlencoded".to_string(),
            "octet-stream" | "binary" => "application/octet-stream".to_string(),
            mime => {
                if mime.contains('/') {
                    mime.to_string()
                } else {
                    format!("application/{}", mime)
                }
            }
        }
    }

    fn parse_parameter(&self, param_str: &str) -> Result<Parameter, ParserError> {
        // Format: name [paramType] [dataType] [required] [description] [attributes...]
        debug!("Parsing parameter: {}", param_str);
        
        // Remove any inline examples for now and handle them separately
        let param_str = if param_str.contains("{example=") {
            let parts: Vec<&str> = param_str.splitn(2, "{example=").collect();
            parts[0].to_string()
        } else {
            param_str.to_string()
        };
        
        // Normalize whitespace - replace tabs and multiple spaces with a single space
        let normalized_param = param_str.replace('\t', " ");
        let normalized_param = Regex::new(r"\s+").unwrap().replace_all(&normalized_param, " ").to_string();
        debug!("Normalized parameter string: {}", normalized_param);
        
        // Split the parameter string by spaces, but keep quoted strings together
        let mut parts = Vec::new();
        let mut current_part = String::new();
        let mut in_quotes = false;
        
        for c in normalized_param.chars() {
            if c == '"' {
                in_quotes = !in_quotes;
                current_part.push(c);
            } else if c == ' ' && !in_quotes {
                if !current_part.is_empty() {
                    parts.push(current_part);
                    current_part = String::new();
                }
            } else {
                current_part.push(c);
            }
        }
        
        if !current_part.is_empty() {
            parts.push(current_part);
        }
        
        // We need at least 5 parts: name, type, dataType, required, and description
        if parts.len() < 5 {
            return Err(ParserError::ParameterParseError(
                format!("Parameter needs at least 5 parts: name, type, dataType, required, description. Got: {}", normalized_param)
            ));
        }
        
        let name = parts[0].clone();
        let param_type = parts[1].clone();
        let data_type = parts[2].clone();
        let required = parts[3].to_lowercase() == "true";
        
        // The description should be quoted
        let description = if parts[4].starts_with('"') && parts[4].ends_with('"') {
            parts[4][1..parts[4].len()-1].to_string()
        } else {
            // Try to reconstruct description from remaining parts if quotes are misplaced
            let desc_parts: Vec<String> = parts.iter().skip(4).cloned().collect();
            let joined = desc_parts.join(" ");
            
            // Extract text between first and last quote if present
            if let (Some(start), Some(end)) = (joined.find('"'), joined.rfind('"')) {
                if start < end {
                    joined[start+1..end].to_string()
                } else {
                    joined
                }
            } else {
                joined
            }
        };
        
        debug!("Parsed parameter parts - Name: {}, Type: {}, DataType: {}, Required: {}, Description: {}",
            name, param_type, data_type, required, description);
                
                let mut parameter = Parameter {
            name,
            in_type: param_type,
            description: Some(description),
                    required: Some(required),
                    schema: None,
                    ..Default::default()
                };
                
                // Handle data type
                if data_type.starts_with("{") && data_type.ends_with("}") {
                    // Object reference
                    let schema_type = data_type[1..data_type.len() - 1].to_string();
                    let parts: Vec<&str> = schema_type.split_whitespace().collect();
                    
                    if parts.len() >= 2 {
                        let type_kind = parts[0];
                        let type_name = parts[1];
                        
                        match type_kind {
                            "object" => {
                                parameter.schema = Some(Schema {
                                    ref_: Some(format!("#/components/schemas/{}", type_name)),
                                    ..Default::default()
                                });
                            },
                            "array" => {
                                parameter.schema = Some(Schema {
                                    type_: Some(serde_json::Value::String("array".to_string())),
                                    items: Some(Box::new(Schema {
                                        ref_: Some(format!("#/components/schemas/{}", type_name)),
                                        ..Default::default()
                                    })),
                                    ..Default::default()
                                });
                            },
                            _ => {
                                return Err(ParserError::ParameterParseError(
                                    format!("Unknown schema type: {}", type_kind)
                                ));
                            }
                        }
                    } else {
                        return Err(ParserError::ParameterParseError(
                            format!("Invalid schema reference format: {}", schema_type)
                        ));
                    }
        } else {
            // Direct model reference (without object keyword)
            if data_type.contains(".") {
                parameter.schema = Some(Schema {
                    ref_: Some(format!("#/components/schemas/{}", data_type)),
                    ..Default::default()
                });
                } else {
                    // Primitive type
                    parameter.schema = Some(Schema {
                        type_: Some(serde_json::Value::String(data_type.to_string())),
                        ..Default::default()
                    });
                    
                    // Handle array types
                    if data_type.starts_with("[]") {
                        parameter.schema = Some(Schema {
                            type_: Some(serde_json::Value::String("array".to_string())),
                            items: Some(Box::new(Schema {
                                type_: Some(serde_json::Value::String(data_type[2..].to_string())),
                                ..Default::default()
                            })),
                            ..Default::default()
                        });
                }
                    }
                }
                
                // Parse optional attributes
        if parts.len() > 5 {
            let attrs_str = parts[5..].join(" ");
            for attr in attrs_str.split_whitespace() {
                        if attr.starts_with("Format(") && attr.ends_with(")") {
                            let format = &attr[7..attr.len() - 1];
                            if let Some(ref mut schema) = parameter.schema {
                                schema.format = Some(format.to_string());
                            }
                        } else if attr.starts_with("Enums(") && attr.ends_with(")") {
                            let enums = &attr[6..attr.len() - 1];
                            let enum_values: Vec<serde_json::Value> = enums.split(',')
                                .map(|s| serde_json::Value::String(s.trim().to_string()))
                                .collect();
                            if let Some(ref mut schema) = parameter.schema {
                                schema.enum_values = Some(enum_values);
                            }
                        } else if attr.starts_with("Default(") && attr.ends_with(")") {
                            let default = &attr[8..attr.len() - 1];
                            if let Some(ref mut schema) = parameter.schema {
                                schema.default = Some(serde_json::Value::String(default.to_string()));
                            }
                        } else if attr.starts_with("Example(") && attr.ends_with(")") {
                            let example = &attr[8..attr.len() - 1];
                            // Try to parse as JSON first
                            if let Ok(json_value) = serde_json::from_str(example) {
                                parameter.example = Some(json_value);
                            } else {
                                parameter.example = Some(serde_json::Value::String(example.to_string()));
                            }
                        }
                    }
                }
                
                // Check for inline example
                if param_str.contains("{example=") {
                    let parts: Vec<&str> = param_str.splitn(2, "{example=").collect();
                    if parts.len() > 1 {
                        let example_part = parts[1];
                        // Extract the example string (remove the trailing '}' if present)
                        let example_content = if example_part.ends_with('}') {
                            &example_part[0..example_part.len() - 1]
                        } else {
                            example_part
                        };
                        
                        // Try to parse the example as JSON
                        if let Ok(example_value) = serde_json::from_str::<serde_json::Value>(example_content) {
                            parameter.example = Some(example_value);
                        } else {
                            debug!("Failed to parse example as JSON: {}", example_content);
                        }
                    }
                }
                
                Ok(parameter)
    }

    fn parse_response(&self, response_str: &str) -> Result<Response, ParserError> {
        // Format: code description [model] [example]
        debug!("Parsing response: {}", response_str);
        
        // Normalize whitespace - replace tabs and multiple spaces with a single space
        let normalized_resp = response_str.replace('\t', " ");
        let normalized_resp = Regex::new(r"\s+").unwrap().replace_all(&normalized_resp, " ").to_string();
        debug!("Normalized response string: {}", normalized_resp);
        
        // First, check if there's an example at the end of the string
        let (response_part, example_part) = if normalized_resp.contains("{example=") {
            let parts: Vec<&str> = normalized_resp.splitn(2, "{example=").collect();
            (parts[0], Some(parts[1]))
        } else {
            (normalized_resp.as_str(), None)
        };
        
        // Split the response into parts
        let parts: Vec<&str> = response_part.split_whitespace().collect();
        
        // We need at least a status code
        if parts.is_empty() {
            return Err(ParserError::ResponseParseError(
                format!("Response needs at least a status code. Got: {}", response_str)
            ));
        }
        
        // Parse the status code
        let code = parts[0].trim();
                let code = if code == "default" {
                    "default".to_string()
                } else {
                    code.to_string()
                };
                
        // Prepare the initial response
                let mut response = Response {
                    code,
            description: String::new(),
                    headers: HashMap::new(),
                    content: HashMap::new(),
                    links: HashMap::new(),
                };
                
        // Check if there's a description part
        let mut description = String::new();
        let mut model_part = None;
        
        // Handle different response formats:
        // 1. "200 {object} Model"
        // 2. "200 {object} Model Description text"
        // 3. "200 Description text {object} Model"
        // 4. "200 Description text"
        
        // Find the model part - it's inside curly braces like {object} or {array}
        let mut i = 1;
        while i < parts.len() {
            let part = parts[i];
            if part.starts_with('{') && part.ends_with('}') {
                model_part = Some(part);
                break;
            }
            i += 1;
        }
        
        // If we found a model part, extract description and model details
        if let Some(model) = model_part {
            let model_index = parts.iter().position(|&p| p == model).unwrap();
            
            // Description could be before or after the model
            if model_index > 1 {
                // Description before model
                description = parts[1..model_index].join(" ");
            } else if model_index + 1 < parts.len() {
                // Description after model+type
                description = parts[model_index+2..].join(" ");
            }
            
            // Check for a model name after the curly braces
            if model_index + 1 < parts.len() {
                let model_type = model.trim_start_matches('{').trim_end_matches('}');
                let model_name = parts[model_index+1];
                
                debug!("Response model type: {}, model name: {}", model_type, model_name);
                
                // Parse the model if provided
                match model_type {
                    "object" => {
                        // Add content type for JSON (the most common)
                        response.content.insert("application/json".to_string(), MediaType {
                            schema: Some(Schema {
                                ref_: Some(format!("#/components/schemas/{}", model_name)),
                                ..Default::default()
                            }),
                            ..Default::default()
                        });
                    },
                    "array" => {
                        response.content.insert("application/json".to_string(), MediaType {
                            schema: Some(Schema {
                                type_: Some(serde_json::Value::String("array".to_string())),
                                items: Some(Box::new(Schema {
                                    ref_: Some(format!("#/components/schemas/{}", model_name)),
                                    ..Default::default()
                                })),
                                ..Default::default()
                            }),
                            ..Default::default()
                        });
                            },
                            _ => {
                        debug!("Unknown model type: {}", model_type);
                    }
                }
            }
        } else {
            // No model, so everything after the status code is the description
            if parts.len() > 1 {
                description = parts[1..].join(" ");
                                }
                            }
        
        // Clean up description - remove quotes if present
        if description.starts_with('"') && description.ends_with('"') {
            description = description[1..description.len()-1].to_string();
        }
        
        response.description = description;
                
                // Process the example if provided
                if let Some(example_str) = example_part {
                    // Extract the example string (remove the trailing '}' if present)
                    let example_content = if example_str.ends_with('}') {
                        &example_str[0..example_str.len() - 1]
                    } else {
                        example_str
                    };
                    
                    // Try to parse the example as JSON
                    if let Ok(example_value) = serde_json::from_str::<serde_json::Value>(example_content) {
                        if let Some(media_type) = response.content.get_mut("application/json") {
                            media_type.example = Some(example_value);
                        } else {
                            let mut media_type = MediaType::default();
                            media_type.example = Some(example_value);
                            response.content.insert("application/json".to_string(), media_type);
                        }
                    } else {
                        debug!("Failed to parse example as JSON: {}", example_content);
                    }
                }
        
        // Ensure we have a default content type for the response
        if response.content.is_empty() {
            response.content.insert("application/json".to_string(), MediaType::default());
                }
                
                Ok(response)
    }
    
    // Extract schema definitions from Go structs in the codebase
    #[allow(dead_code)]
    pub fn extract_struct_schemas(&self, file_paths: &[PathBuf]) -> HashMap<String, Schema> {
        use regex::Regex;
        let mut schemas: HashMap<String, Schema> = HashMap::new();
        
        // Regular expressions for struct parsing
        let struct_regex = Regex::new(r"type\s+(\w+)\s+struct\s*\{").unwrap();
        let field_regex = Regex::new(r#"^\s*(\w+)\s+(\w+(?:\[\])?\*?)(?:\s+`[^`]*`)?.*$"#).unwrap();
        
        // Track package names from imports to handle qualified model names
        let mut package_imports: HashMap<String, String> = HashMap::new();
        let import_regex = Regex::new(r#"import\s+\(\s*((?:[^()]*\n)*)\s*\)"#).unwrap();
        let import_line_regex = Regex::new(r#"\s*(?:([a-zA-Z0-9_]+)\s+)?"([^"]+)""#).unwrap();
        let single_import_regex = Regex::new(r#"import\s+(?:([a-zA-Z0-9_]+)\s+)?"([^"]+)""#).unwrap();
        
        for file_path in file_paths {
            debug!("Extracting schemas from file: {:?}", file_path);
            
            if let Ok(content) = std::fs::read_to_string(file_path) {
                // Extract package imports first
                // Multi-line imports
                if let Some(caps) = import_regex.captures(&content) {
                    if let Some(import_block) = caps.get(1) {
                        for line in import_block.as_str().lines() {
                            if let Some(m) = import_line_regex.captures(line) {
                                let alias = m.get(1).map_or_else(
                                    || {
                                        // If no alias, use the last part of the path
                                        let path = m.get(2).unwrap().as_str();
                                        path.split('/').last().unwrap_or(path).to_string()
                                    },
                                    |a| a.as_str().to_string(),
                                );
                                let path = m.get(2).unwrap().as_str().to_string();
                                package_imports.insert(alias, path);
                            }
                        }
                    }
                }
                
                // Single-line imports
                for caps in single_import_regex.captures_iter(&content) {
                    let alias = caps.get(1).map_or_else(
                        || {
                            // If no alias, use the last part of the path
                            let path = caps.get(2).unwrap().as_str();
                            path.split('/').last().unwrap_or(path).to_string()
                        },
                        |a| a.as_str().to_string(),
                    );
                    let path = caps.get(2).unwrap().as_str().to_string();
                    package_imports.insert(alias, path);
                }
                
                // Now parse structs
                let lines: Vec<&str> = content.lines().collect();
                let mut i = 0;
                
                while i < lines.len() {
                    if let Some(captures) = struct_regex.captures(lines[i]) {
                        let struct_name = captures.get(1).unwrap().as_str();
                        debug!("Found struct for schema: {}", struct_name);
                        
                        let mut schema = Schema {
                            type_: Some(serde_json::Value::String("object".to_string())),
                            properties: HashMap::new(),
                            ..Default::default()
                        };
                        
                        let mut required_fields = Vec::new();
                        let mut j = i + 1;
                        
                        // Parse fields until we reach the closing brace
                        while j < lines.len() && !lines[j].trim().starts_with("}") {
                            if let Some(field_captures) = field_regex.captures(lines[j]) {
                                let field_name = field_captures.get(1).unwrap().as_str();
                                let field_type = field_captures.get(2).unwrap().as_str();
                                
                                debug!("  Field: {} with type: {}", field_name, field_type);
                                
                                // Convert Go types to OpenAPI types
                                let field_schema = match field_type {
                                    "string" => Schema {
                                        type_: Some(serde_json::Value::String("string".to_string())),
                                        ..Default::default()
                                    },
                                    "int" | "int8" | "int16" | "int32" | "int64" | "uint" | "uint8" | "uint16" | "uint32" | "uint64" => Schema {
                                        type_: Some(serde_json::Value::String("integer".to_string())),
                                        ..Default::default()
                                    },
                                    "float32" | "float64" => Schema {
                                        type_: Some(serde_json::Value::String("number".to_string())),
                                        ..Default::default()
                                    },
                                    "bool" => Schema {
                                        type_: Some(serde_json::Value::String("boolean".to_string())),
                                        ..Default::default()
                                    },
                                    t if t.starts_with("[]") => {
                                        // Array type
                                        let item_type = &t[2..]; // Remove "[]" prefix
                                        
                                        // Handle qualified references in arrays, e.g. []packageName.Type
                                        if item_type.contains('.') {
                                            let parts: Vec<&str> = item_type.split('.').collect();
                                            if parts.len() == 2 {
                                                let package_alias = parts[0];
                                                let type_name = parts[1];
                                                
                                                // Full qualified name for the reference
                                                let ref_name = format!("{}.{}", package_alias, type_name);
                                                
                                                Schema {
                                                    type_: Some(serde_json::Value::String("array".to_string())),
                                                    items: Some(Box::new(Schema {
                                                        ref_: Some(format!("#/components/schemas/{}", ref_name)),
                                                        ..Default::default()
                                                    })),
                                                    ..Default::default()
                                                }
                                            } else {
                                                Schema {
                                                    type_: Some(serde_json::Value::String("array".to_string())),
                                                    items: Some(Box::new(Schema {
                                                        ref_: Some(format!("#/components/schemas/{}", item_type)),
                                                        ..Default::default()
                                                    })),
                                                    ..Default::default()
                                                }
                                            }
                                        } else {
                                        let item_schema = match item_type {
                                            "string" => Schema {
                                                type_: Some(serde_json::Value::String("string".to_string())),
                                                ..Default::default()
                                            },
                                            "int" | "int8" | "int16" | "int32" | "int64" | "uint" | "uint8" | "uint16" | "uint32" | "uint64" => Schema {
                                                type_: Some(serde_json::Value::String("integer".to_string())),
                                                ..Default::default()
                                            },
                                            "float32" | "float64" => Schema {
                                                type_: Some(serde_json::Value::String("number".to_string())),
                                                ..Default::default()
                                            },
                                            "bool" => Schema {
                                                type_: Some(serde_json::Value::String("boolean".to_string())),
                                                ..Default::default()
                                            },
                                            _ => {
                                                // Reference to another type
                                                Schema {
                                                    ref_: Some(format!("#/components/schemas/{}", item_type)),
                                                    ..Default::default()
                                                }
                                            }
                                        };
                                        
                                        Schema {
                                            type_: Some(serde_json::Value::String("array".to_string())),
                                            items: Some(Box::new(item_schema)),
                                            ..Default::default()
                                            }
                                        }
                                    },
                                    t if t.starts_with("*") => {
                                        // Pointer type (optional)
                                        let base_type = &t[1..]; // Remove "*" prefix
                                        
                                        // Handle qualified references in pointers, e.g. *packageName.Type
                                        if base_type.contains('.') {
                                            let parts: Vec<&str> = base_type.split('.').collect();
                                            if parts.len() == 2 {
                                                let package_alias = parts[0];
                                                let type_name = parts[1];
                                                
                                                // Full qualified name for the reference
                                                let ref_name = format!("{}.{}", package_alias, type_name);
                                                
                                                Schema {
                                                    ref_: Some(format!("#/components/schemas/{}", ref_name)),
                                                    ..Default::default()
                                                }
                                            } else {
                                                Schema {
                                                    ref_: Some(format!("#/components/schemas/{}", base_type)),
                                                    ..Default::default()
                                                }
                                            }
                                        } else {
                                        match base_type {
                                            "string" => Schema {
                                                type_: Some(serde_json::Value::String("string".to_string())),
                                                ..Default::default()
                                            },
                                            "int" | "int8" | "int16" | "int32" | "int64" | "uint" | "uint8" | "uint16" | "uint32" | "uint64" => Schema {
                                                type_: Some(serde_json::Value::String("integer".to_string())),
                                                ..Default::default()
                                            },
                                            "float32" | "float64" => Schema {
                                                type_: Some(serde_json::Value::String("number".to_string())),
                                                ..Default::default()
                                            },
                                            "bool" => Schema {
                                                type_: Some(serde_json::Value::String("boolean".to_string())),
                                                ..Default::default()
                                            },
                                            _ => {
                                                // Reference to another type
                                                Schema {
                                                    ref_: Some(format!("#/components/schemas/{}", base_type)),
                                                    ..Default::default()
                                                    }
                                                }
                                            }
                                        }
                                    },
                                    t if t.contains('.') => {
                                        // This is a reference to a type in another package (e.g., user.User)
                                        let parts: Vec<&str> = t.split('.').collect();
                                        if parts.len() == 2 {
                                            let package_alias = parts[0];
                                            let type_name = parts[1];
                                            
                                            // Full qualified name for the reference
                                            let ref_name = format!("{}.{}", package_alias, type_name);
                                            
                                            Schema {
                                                ref_: Some(format!("#/components/schemas/{}", ref_name)),
                                                ..Default::default()
                                            }
                                        } else {
                                            Schema {
                                                ref_: Some(format!("#/components/schemas/{}", t)),
                                                ..Default::default()
                                            }
                                        }
                                    },
                                    _ => {
                                        // Reference to another type
                                        Schema {
                                            ref_: Some(format!("#/components/schemas/{}", field_type)),
                                            ..Default::default()
                                        }
                                    }
                                };
                                
                                // Check if the field is required (not a pointer type)
                                if !field_type.starts_with("*") {
                                    required_fields.push(field_name.to_string());
                                }
                                
                                schema.properties.insert(field_name.to_string(), Box::new(field_schema));
                            }
                            j += 1;
                        }
                        
                        // Add required fields if any
                        if !required_fields.is_empty() {
                            schema.required = Some(required_fields);
                        }
                        
                        // Add the schema with different names for better reference resolution
                        
                        // 1. Add with the simple name (e.g., "User")
                        schemas.insert(struct_name.to_string(), schema.clone());
                        
                        // 2. Also add package-qualified names for all known package imports
                        // This handles references like "userModel.User" by creating schemas 
                        // with both names "User" and "userModel.User"
                        for (package_alias, _) in &package_imports {
                            let qualified_name = format!("{}.{}", package_alias, struct_name);
                            debug!("Adding schema with qualified name: {}", qualified_name);
                            schemas.insert(qualified_name, schema.clone());
                        }
                        
                        i = j;
                    }
                    i += 1;
                }
            }
        }
        
        schemas
    }

    // Add a new method to set response examples
    #[allow(dead_code)]
    fn set_response_examples(&self, 
                         response: &mut Response, 
                         struct_examples: &HashMap<String, HashMap<String, serde_json::Value>>) {
        // Try to add examples from struct fields if available
        for (_content_type, media_type) in &mut response.content {
            if let Some(schema) = &media_type.schema {
                if let Some(ref_) = &schema.ref_ {
                    // Extract the model name from the reference
                    let full_type_name = ref_.split('/').last().unwrap_or("");
                    debug!("Looking for response examples for model: {}", full_type_name);
                    
                    // Try different variations of the model name to find examples
                    // 1. Full reference name (e.g., "userModel.UserLoggedInResponse")
                    // 2. Just the type name portion (e.g., "UserLoggedInResponse")
                    let possible_names = vec![
                        full_type_name.to_string(),
                        if full_type_name.contains('.') {
                            full_type_name.split('.').last().unwrap_or("").to_string()
                        } else {
                            full_type_name.to_string()
                        }
                    ];
                    
                    for name in possible_names {
                        debug!("Checking for examples with name: {}", name);
                        if let Some(examples) = struct_examples.get(&name) {
                            debug!("Found examples for response model: {}", name);
                            let example_value = serde_json::to_value(examples).unwrap_or(serde_json::Value::Null);
                            if !example_value.is_null() && media_type.example.is_none() {
                                debug!("Setting example for response: {:?}", example_value);
                                media_type.example = Some(example_value);
                                break;
                            }
                        }
                    }
                } else if let Some(items) = &schema.items {
                    // Handle array items that have references
                    if let Some(ref_) = &items.ref_ {
                        let item_type_name = ref_.split('/').last().unwrap_or("");
                        debug!("Looking for response array item examples for model: {}", item_type_name);
                        
                        let possible_names = vec![
                            item_type_name.to_string(),
                            if item_type_name.contains('.') {
                                item_type_name.split('.').last().unwrap_or("").to_string()
                            } else {
                                item_type_name.to_string()
                            }
                        ];
                        
                        for name in possible_names {
                            if let Some(examples) = struct_examples.get(&name) {
                                debug!("Found examples for array item model: {}", name);
                                let item_example = serde_json::to_value(examples).unwrap_or(serde_json::Value::Null);
                                if !item_example.is_null() && media_type.example.is_none() {
                                    // Create an array example
                                    let array_example = serde_json::Value::Array(vec![item_example]);
                                    media_type.example = Some(array_example);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // New method to collect model names that are referenced in annotations
    #[allow(dead_code)]
    fn collect_referenced_models(&self, file_paths: &[PathBuf]) -> HashSet<String> {
        let mut referenced_models = HashSet::new();
        
        // Patterns to match model references in annotations
        let param_regex = Regex::new(r"@Param\s+\w+\s+body\s+([a-zA-Z0-9_.]+)").unwrap();
        let request_body_regex = Regex::new(r"@RequestBody\s+.*\{object\}\s+([a-zA-Z0-9_.]+)").unwrap();
        // Make the response regex more flexible to handle various whitespace patterns
        let response_regex = Regex::new(r"@(?:Success|Failure|Response)\s+\d+\s+\{(?:object|array)\}\s*([a-zA-Z0-9_.]+)").unwrap();
        
        for file_path in file_paths {
            if let Ok(content) = std::fs::read_to_string(file_path) {
                // Find all model references in @Param annotations
                for cap in param_regex.captures_iter(&content) {
                    if let Some(m) = cap.get(1) {
                        let model_name = m.as_str().to_string();
                        referenced_models.insert(model_name.clone());
                        
                        // Also add the bare model name without package prefix
                        if model_name.contains('.') {
                            if let Some(base_name) = model_name.split('.').last() {
                                referenced_models.insert(base_name.to_string());
                            }
                        }
                    }
                }
                
                // Find all model references in @RequestBody annotations
                for cap in request_body_regex.captures_iter(&content) {
                    if let Some(m) = cap.get(1) {
                        let model_name = m.as_str().to_string();
                        referenced_models.insert(model_name.clone());
                        
                        // Also add the bare model name without package prefix
                        if model_name.contains('.') {
                            if let Some(base_name) = model_name.split('.').last() {
                                referenced_models.insert(base_name.to_string());
                            }
                        }
                    }
                }
                
                // Find all model references in @Success, @Failure, and @Response annotations
                for cap in response_regex.captures_iter(&content) {
                    if let Some(m) = cap.get(1) {
                        let model_name = m.as_str().to_string();
                        referenced_models.insert(model_name.clone());
                        
                        // Also add the bare model name without package prefix
                        if model_name.contains('.') {
                            if let Some(base_name) = model_name.split('.').last() {
                                referenced_models.insert(base_name.to_string());
                            }
                        }
                    }
                }
            }
        }
        
        debug!("Found referenced models: {:?}", referenced_models);
        referenced_models
    }
    
    // Modified method to extract schemas only for referenced models
    fn extract_referenced_schemas(&self, file_paths: &[PathBuf], referenced_models: &HashSet<String>) -> HashMap<String, Schema> {
        use regex::Regex;
        let mut schemas: HashMap<String, Schema> = HashMap::new();
        let mut schema_dependencies = HashMap::new();
        
        // Regular expressions for struct parsing
        let struct_regex = Regex::new(r"type\s+(\w+)\s+struct\s*\{").unwrap();
        let field_regex = Regex::new(r#"^\s*(\w+)\s+(\*?\w+(?:\.\w+)?(?:\[\])?(?:\[\])?)(?:\s+`[^`]*`)?.*$"#).unwrap();
        
        // Track package names from imports to handle qualified model names
        let mut package_imports: HashMap<String, String> = HashMap::new();
        let import_regex = Regex::new(r#"import\s+\(\s*((?:[^()]*\n)*)\s*\)"#).unwrap();
        let import_line_regex = Regex::new(r#"\s*(?:([a-zA-Z0-9_]+)\s+)?"([^"]+)""#).unwrap();
        let single_import_regex = Regex::new(r#"import\s+(?:([a-zA-Z0-9_]+)\s+)?"([^"]+)""#).unwrap();
        
        // Track which models we need to process
        let mut models_to_process = referenced_models.clone();
        let mut processed_models = HashSet::new();
        
        // Add some common response types that might be referenced
        // These are basic schema definitions for commonly referenced types
        self.add_common_schemas(&mut schemas);
        
        while !models_to_process.is_empty() {
            let mut new_dependencies = HashSet::new();
            
            for file_path in file_paths {
                debug!("Looking for referenced models in file: {:?}", file_path);
                
                if let Ok(content) = std::fs::read_to_string(file_path) {
                    // Extract package imports first
                    // Multi-line imports
                    if let Some(caps) = import_regex.captures(&content) {
                        if let Some(import_block) = caps.get(1) {
                            for line in import_block.as_str().lines() {
                                if let Some(m) = import_line_regex.captures(line) {
                                    let alias = m.get(1).map_or_else(
                                        || {
                                            // If no alias, use the last part of the path
                                            let path = m.get(2).unwrap().as_str();
                                            path.split('/').last().unwrap_or(path).to_string()
                                        },
                                        |a| a.as_str().to_string(),
                                    );
                                    let path = m.get(2).unwrap().as_str().to_string();
                                    package_imports.insert(alias, path);
                                }
                            }
                        }
                    }
                    
                    // Single-line imports
                    for caps in single_import_regex.captures_iter(&content) {
                        let alias = caps.get(1).map_or_else(
                            || {
                                // If no alias, use the last part of the path
                                let path = caps.get(2).unwrap().as_str();
                                path.split('/').last().unwrap_or(path).to_string()
                            },
                            |a| a.as_str().to_string(),
                        );
                        let path = caps.get(2).unwrap().as_str().to_string();
                        package_imports.insert(alias, path);
                    }
                    
                    // Now parse structs
                    let lines: Vec<&str> = content.lines().collect();
                    let mut i = 0;
                    
                    while i < lines.len() {
                        if let Some(captures) = struct_regex.captures(lines[i]) {
                            let struct_name = captures.get(1).unwrap().as_str();
                            
                            // Check if this struct is one we need to process
                            // Look for both simple names and qualified names
                            let is_referenced = models_to_process.contains(struct_name) ||
                                               models_to_process.iter().any(|m| {
                                                  m.contains('.') && m.split('.').last().unwrap_or("") == struct_name
                                               });
                            
                            if is_referenced && !processed_models.contains(struct_name) {
                                debug!("Processing referenced struct: {}", struct_name);
                                processed_models.insert(struct_name.to_string());
                                
                                let mut schema = Schema {
                                    type_: Some(serde_json::Value::String("object".to_string())),
                                    properties: HashMap::new(),
                                    ..Default::default()
                                };
                                
                                let mut required_fields = Vec::new();
                                let mut field_dependencies = HashSet::new();
                                let mut j = i + 1;
                                
                                // Parse fields until we reach the closing brace
                                while j < lines.len() && !lines[j].trim().starts_with('}') {
                                    if let Some(field_captures) = field_regex.captures(lines[j]) {
                                        let field_name = field_captures.get(1).unwrap().as_str();
                                        let field_type = field_captures.get(2).unwrap().as_str();
                                        
                                        debug!("  Field: {} with type: {}", field_name, field_type);
                                        
                                        // Track dependencies in this field
                                        let field_schema = self.convert_go_type_to_schema(field_type);
                                        
                                        // Add any referenced types to our dependencies
                                        self.collect_field_dependencies(field_type, &mut field_dependencies);
                                        
                                        // Check if the field is required (not a pointer type)
                                        if !field_type.starts_with("*") {
                                            required_fields.push(field_name.to_string());
                                        }
                                        
                                        schema.properties.insert(field_name.to_string(), Box::new(field_schema));
                                    }
                                    j += 1;
                                }
                                
                                // Add required fields if any
                                if !required_fields.is_empty() {
                                    schema.required = Some(required_fields);
                                }
                                
                                // Add the schema with different names for better reference resolution
                                
                                // 1. Add with the simple name (e.g., "User")
                                schemas.insert(struct_name.to_string(), schema.clone());
                                
                                // 2. Also add package-qualified names for all known package imports
                                // This handles references like "userModel.User" by creating schemas 
                                // with both names "User" and "userModel.User"
                                for (package_alias, _) in &package_imports {
                                    let qualified_name = format!("{}.{}", package_alias, struct_name);
                                    debug!("Adding schema with qualified name: {}", qualified_name);
                                    schemas.insert(qualified_name, schema.clone());
                                }
                                
                                // Store dependencies for this model
                                schema_dependencies.insert(struct_name.to_string(), field_dependencies);
                                
                                i = j;
                            }
                        }
                        i += 1;
                    }
                }
            }
            
            // Add all new dependencies to process
            models_to_process.clear();
            for (_, deps) in &schema_dependencies {
                for dep in deps {
                    if !processed_models.contains(dep) && !dep.contains("[]") {
                        models_to_process.insert(dep.clone());
                        new_dependencies.insert(dep.clone());
                    }
                }
            }
            
            // If no new dependencies were found, we're done
            if new_dependencies.is_empty() {
                break;
            }
            
            debug!("Added {} new dependencies to process", new_dependencies.len());
        }
        
        // Make sure all referenced types have schemas
        for model_name in referenced_models {
            if !schemas.contains_key(model_name) {
                debug!("No schema definition found for {}, adding a basic one", model_name);
                
                // Create a basic schema for this model
                let basic_schema = Schema {
                    type_: Some(serde_json::Value::String("object".to_string())),
                    ..Default::default()
                };
                
                schemas.insert(model_name.clone(), basic_schema);
            }
        }
        
        schemas
    }
    
    // Add common schema definitions that are often referenced in Go APIs
    fn add_common_schemas(&self, schemas: &mut HashMap<String, Schema>) {
        // Common response types
        
        // Generic API Response
        schemas.insert(
            "response.ApiResponse".to_string(), 
            Schema {
                type_: Some(serde_json::Value::String("object".to_string())),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("Status".to_string(), Box::new(Schema {
                        type_: Some(serde_json::Value::String("string".to_string())),
                        ..Default::default()
                    }));
                    props.insert("Code".to_string(), Box::new(Schema {
                        type_: Some(serde_json::Value::String("string".to_string())),
                        ..Default::default()
                    }));
                    props.insert("Message".to_string(), Box::new(Schema {
                        type_: Some(serde_json::Value::String("string".to_string())),
                        ..Default::default()
                    }));
                    props.insert("Data".to_string(), Box::new(Schema {
                        type_: Some(serde_json::Value::String("object".to_string())),
                        ..Default::default()
                    }));
                    props
                },
                ..Default::default()
            }
        );
        
        // Regular Response
        schemas.insert(
            "response.Response".to_string(), 
            Schema {
                type_: Some(serde_json::Value::String("object".to_string())),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("Status".to_string(), Box::new(Schema {
                        type_: Some(serde_json::Value::String("string".to_string())),
                        ..Default::default()
                    }));
                    props.insert("Code".to_string(), Box::new(Schema {
                        type_: Some(serde_json::Value::String("string".to_string())),
                        ..Default::default()
                    }));
                    props.insert("Message".to_string(), Box::new(Schema {
                        type_: Some(serde_json::Value::String("string".to_string())),
                        ..Default::default()
                    }));
                    props.insert("Data".to_string(), Box::new(Schema {
                        type_: Some(serde_json::Value::String("object".to_string())),
                        ..Default::default()
                    }));
                    props
                },
                ..Default::default()
            }
        );
        
        // OpenAPI Response
        schemas.insert(
            "response.OpenApiResponse".to_string(), 
            Schema {
                type_: Some(serde_json::Value::String("object".to_string())),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("Status".to_string(), Box::new(Schema {
                        type_: Some(serde_json::Value::String("string".to_string())),
                        ..Default::default()
                    }));
                    props.insert("Code".to_string(), Box::new(Schema {
                        type_: Some(serde_json::Value::String("string".to_string())),
                        ..Default::default()
                    }));
                    props.insert("Message".to_string(), Box::new(Schema {
                        type_: Some(serde_json::Value::String("string".to_string())),
                        ..Default::default()
                    }));
                    props.insert("Data".to_string(), Box::new(Schema {
                        type_: Some(serde_json::Value::String("object".to_string())),
                        ..Default::default()
                    }));
                    props
                },
                ..Default::default()
            }
        );
        
        // Error Response
        schemas.insert(
            "response.OpenApiErrorNonSnap".to_string(), 
            Schema {
                type_: Some(serde_json::Value::String("object".to_string())),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("Status".to_string(), Box::new(Schema {
                        type_: Some(serde_json::Value::String("string".to_string())),
                        ..Default::default()
                    }));
                    props.insert("Code".to_string(), Box::new(Schema {
                        type_: Some(serde_json::Value::String("string".to_string())),
                        ..Default::default()
                    }));
                    props.insert("Message".to_string(), Box::new(Schema {
                        type_: Some(serde_json::Value::String("string".to_string())),
                        ..Default::default()
                    }));
                    props
                },
                ..Default::default()
            }
        );
    }
    
    // Helper to convert Go types to OpenAPI schema
    fn convert_go_type_to_schema(&self, field_type: &str) -> Schema {
        match field_type {
            "string" => Schema {
                type_: Some(serde_json::Value::String("string".to_string())),
                ..Default::default()
            },
            "int" | "int8" | "int16" | "int32" | "int64" | "uint" | "uint8" | "uint16" | "uint32" | "uint64" => Schema {
                type_: Some(serde_json::Value::String("integer".to_string())),
                ..Default::default()
            },
            "float32" | "float64" => Schema {
                type_: Some(serde_json::Value::String("number".to_string())),
                ..Default::default()
            },
            "bool" => Schema {
                type_: Some(serde_json::Value::String("boolean".to_string())),
                ..Default::default()
            },
            t if t.starts_with("[]") => {
                // Array type
                let item_type = &t[2..]; // Remove "[]" prefix
                
                // Handle qualified references in arrays, e.g. []packageName.Type
                if item_type.contains('.') {
                    let parts: Vec<&str> = item_type.split('.').collect();
                    if parts.len() == 2 {
                        let package_alias = parts[0];
                        let type_name = parts[1];
                        
                        // Full qualified name for the reference
                        let ref_name = format!("{}.{}", package_alias, type_name);
                        
                        Schema {
                            type_: Some(serde_json::Value::String("array".to_string())),
                            items: Some(Box::new(Schema {
                                ref_: Some(format!("#/components/schemas/{}", ref_name)),
                                ..Default::default()
                            })),
                            ..Default::default()
                        }
                    } else {
                        Schema {
                            type_: Some(serde_json::Value::String("array".to_string())),
                            items: Some(Box::new(Schema {
                                ref_: Some(format!("#/components/schemas/{}", item_type)),
                                ..Default::default()
                            })),
                            ..Default::default()
                        }
                    }
                } else {
                    let item_schema = self.convert_go_type_to_schema(item_type);
                    
                    Schema {
                        type_: Some(serde_json::Value::String("array".to_string())),
                        items: Some(Box::new(item_schema)),
                        ..Default::default()
                    }
                }
            },
            t if t.starts_with("*") => {
                // Pointer type (optional)
                let base_type = &t[1..]; // Remove "*" prefix
                self.convert_go_type_to_schema(base_type)
            },
            t if t.contains('.') => {
                // This is a reference to a type in another package (e.g., user.User)
                let parts: Vec<&str> = t.split('.').collect();
                if parts.len() == 2 {
                    let package_alias = parts[0];
                    let type_name = parts[1];
                    
                    // Full qualified name for the reference
                    let ref_name = format!("{}.{}", package_alias, type_name);
                    
                    Schema {
                        ref_: Some(format!("#/components/schemas/{}", ref_name)),
                        ..Default::default()
                    }
                } else {
                    Schema {
                        ref_: Some(format!("#/components/schemas/{}", t)),
                        ..Default::default()
                    }
                }
            },
            _ => {
                // Reference to another type
                Schema {
                    ref_: Some(format!("#/components/schemas/{}", field_type)),
                    ..Default::default()
                }
            }
        }
    }
    
    // Helper to collect dependencies from a field type
    fn collect_field_dependencies(&self, field_type: &str, dependencies: &mut HashSet<String>) {
        match field_type {
            "string" | "int" | "int8" | "int16" | "int32" | "int64" | 
            "uint" | "uint8" | "uint16" | "uint32" | "uint64" |
            "float32" | "float64" | "bool" => {
                // Basic types have no dependencies
            },
            t if t.starts_with("[]") => {
                // Array type - collect dependencies from item type
                let item_type = &t[2..]; // Remove "[]" prefix
                self.collect_field_dependencies(item_type, dependencies);
            },
            t if t.starts_with("*") => {
                // Pointer type - collect dependencies from base type
                let base_type = &t[1..]; // Remove "*" prefix
                self.collect_field_dependencies(base_type, dependencies);
            },
            t if t.contains('.') => {
                // Reference to a type in another package
                let parts: Vec<&str> = t.split('.').collect();
                if parts.len() == 2 {
                    let type_name = parts[1];
                    dependencies.insert(type_name.to_string());
                    dependencies.insert(t.to_string()); // Also add the full qualified name
                }
            },
            _ => {
                // Reference to another type in the same package
                dependencies.insert(field_type.to_string());
            }
        }
    }
} 