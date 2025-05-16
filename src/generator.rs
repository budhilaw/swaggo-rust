use anyhow::{Context, Result};
use log::{debug, info};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::Write,
    path::Path,
};

use crate::models::{
    Components, Contact, ExternalDocs, Info, License, MediaType, OAuthFlows, Operation, OpenAPI, 
    ParsedApiInfo, ParsedOperation, PathItem, Schema, SecurityScheme, Server, Tag
};

// Default maximum file size (5MB)
const DEFAULT_MAX_FILE_SIZE: usize = 5 * 1024 * 1024;
/// Generates OpenAPI 3.1.1 documentation from parsed API info and operations
pub struct Generator {
    api_info: ParsedApiInfo,
    operations: Vec<ParsedOperation>,
    schemas: HashMap<String, Schema>,
    max_file_size: usize,
    openapi_version: String,
}

impl Generator {
    /// Create a new Generator instance
    pub fn new(api_info: ParsedApiInfo, operations: Vec<ParsedOperation>, schemas: HashMap<String, Schema>, openapi_version: String) -> Self {
        Self {
            api_info,
            operations,
            schemas,
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            openapi_version,
        }
    }
    
    /// Create a new Generator instance with custom maximum file size
    pub fn new_with_max_file_size(
        api_info: ParsedApiInfo, 
        operations: Vec<ParsedOperation>, 
        schemas: HashMap<String, Schema>,
        max_file_size: usize,
        openapi_version: String,
    ) -> Self {
        Self {
            api_info,
            operations,
            schemas,
            max_file_size,
            openapi_version,
        }
    }
    
    /// Generate OpenAPI 3.1.1 documentation in the specified output formats
    pub fn generate(&self, output_dir: impl AsRef<Path>, output_types: &[String]) -> Result<()> {
        let output_dir = output_dir.as_ref();
        
        // Create output directory if it doesn't exist
        fs::create_dir_all(output_dir)
            .context(format!("Failed to create output directory: {:?}", output_dir))?;
        
        // Build the OpenAPI document
        let openapi = self.build_openapi_doc();
        
        // Generate the specified output types
        for output_type in output_types {
            match output_type.as_str() {
                "json" => self.generate_json(output_dir, &openapi)?,
                "yaml" => self.generate_yaml(output_dir, &openapi)?,
                "go" => self.generate_go(output_dir, &openapi)?,
                "ui" => {
                    // Generate both Swagger UI HTML template and handler
                    self.generate_swagger_ui(output_dir)?;
                    self.generate_swagger_handler(output_dir)?;
                },
                _ => debug!("Unknown output type: {}", output_type),
            }
        }
        
        Ok(())
    }
    
    /// Build a complete OpenAPI 3.1.1 document
    fn build_openapi_doc(&self) -> OpenAPI {
        let mut openapi = OpenAPI {
            openapi: self.openapi_version.clone(),
            info: self.api_info.info.clone(),
            paths: HashMap::new(),
            components: None, // Will be set later
            ..Default::default()
        };
        
        // Convert servers from legacy host/basePath/schemes
        if self.api_info.servers.is_empty() {
            let mut servers = Vec::new();
            
            if let Some(host) = &self.api_info.host {
                for scheme in &self.api_info.schemes {
                    let url = format!(
                        "{}://{}{}",
                        scheme,
                        host,
                        self.api_info.base_path.as_deref().unwrap_or("")
                    );
                    
                    servers.push(Server {
                        url,
                        description: None,
                        variables: HashMap::new(),
                    });
                }
            }
            
            if !servers.is_empty() {
                openapi.servers = Some(servers);
            }
        } else {
            openapi.servers = Some(self.api_info.servers.clone());
        }
        
        // Always create a components object, even if empty
        let mut components = Components::default();
        
        // Add security definitions to components
        if !self.api_info.security_definitions.is_empty() {
            components.securitySchemes = self.api_info.security_definitions.clone();
        }
        
        // Add schemas to components
        components.schemas = self.schemas.clone();
        
        // Even if schemas is empty, explicitly ensure it exists and doesn't get skipped
        if components.schemas.is_empty() {
            debug!("No schemas found, creating empty schemas object");
            components.ensure_schemas_exists();
        }
        
        // Fix schema references in components
        self.fix_schema_references(&mut components);
        
        // Set the components in the OpenAPI document (always set it, even if empty)
        openapi.components = Some(components);
        
        // Copy tags and external docs
        openapi.tags = self.api_info.tags.clone();
        openapi.externalDocs = self.api_info.external_docs.clone();
        
        // Copy global security requirements
        if !self.api_info.security.is_empty() {
            openapi.security = self.api_info.security.clone();
        }
        
        // Add operations to paths
        for operation in &self.operations {
            let path = operation.path.clone();
            let method = operation.method.clone();
            
            let path_item = openapi.paths.entry(path).or_insert_with(PathItem::default);
            
            // Convert legacy consumes/produces to requestBody/responses content
            let mut op = operation.operation.clone();
            
            // Fix schema references in the operation
            self.fix_operation_references(&mut op);
            
            // Add path parameters if needed
            path_item.parameters = operation.operation.parameters.iter()
                .filter(|p| p.in_type == "path")
                .cloned()
                .collect();
            
            match method.as_str() {
                "get" => path_item.get = Some(op),
                "post" => path_item.post = Some(op),
                "put" => path_item.put = Some(op),
                "delete" => path_item.delete = Some(op),
                "options" => path_item.options = Some(op),
                "head" => path_item.head = Some(op),
                "patch" => path_item.patch = Some(op),
                "trace" => path_item.trace = Some(op),
                _ => debug!("Unknown HTTP method: {}", method),
            }
        }
        
        openapi
    }
    
    /// Fix references in schemas to use the correct format for OpenAPI 3.1.1
    fn fix_schema_references(&self, components: &mut Components) {
        // Fix references in all schemas
        for (_, schema) in components.schemas.iter_mut() {
            self.fix_references_in_schema(schema);
        }
    }
    
    /// Fix references in an operation to use the correct format for OpenAPI 3.1.1
    fn fix_operation_references(&self, operation: &mut Operation) {
        // Fix references in parameters
        for param in operation.parameters.iter_mut() {
            if let Some(schema) = &mut param.schema {
                self.fix_references_in_schema(schema);
            }
        }
        
        // Fix references in request body
        if let Some(request_body) = &mut operation.requestBody {
            for (_, media_type) in request_body.content.iter_mut() {
                if let Some(schema) = &mut media_type.schema {
                    self.fix_references_in_schema(schema);
                }
            }
        }
        
        // Fix references in responses
        for (_, response) in operation.responses.iter_mut() {
            for (_, media_type) in response.content.iter_mut() {
                if let Some(schema) = &mut media_type.schema {
                    self.fix_references_in_schema(schema);
                }
            }
        }
    }
    
    /// Recursively fix references in a schema
    fn fix_references_in_schema(&self, schema: &mut Schema) {
        // Fix $ref format if present
        if let Some(ref_) = &mut schema.ref_ {
            if ref_.starts_with("#/components/schemas/") {
                // Format is already correct
                debug!("Reference already correctly formatted: {}", ref_);
            } else if ref_.starts_with("/components/schemas/") {
                // Add the missing hash
                *ref_ = format!("#{}", ref_);
                debug!("Fixed reference format by adding #: {}", ref_);
            } else if !ref_.contains('/') {
                // It's just a schema name, format it correctly
                *ref_ = format!("#/components/schemas/{}", ref_);
                debug!("Converted simple schema name to full reference: {}", ref_);
            } else {
                // Some other reference format - try to fix it
                debug!("Unknown reference format: {}, attempting to fix", ref_);
                if ref_.ends_with(".json") || ref_.ends_with(".yaml") {
                    // External file reference, leave it as is
                } else if let Some(schema_name) = ref_.split('/').last() {
                    // Extract the schema name and create a proper reference
                    *ref_ = format!("#/components/schemas/{}", schema_name);
                    debug!("Extracted schema name and reformatted reference: {}", ref_);
                }
            }
        }
        
        // Fix references in items (for arrays)
        if let Some(items) = &mut schema.items {
            self.fix_references_in_schema(items);
        }
        
        // Fix references in properties (for objects)
        for (_, property) in schema.properties.iter_mut() {
            self.fix_references_in_schema(property);
        }
        
        // Fix references in allOf, anyOf, oneOf
        if let Some(all_of) = &mut schema.allOf {
            for schema_item in all_of.iter_mut() {
                self.fix_references_in_schema(schema_item);
            }
        }
        
        if let Some(any_of) = &mut schema.anyOf {
            for schema_item in any_of.iter_mut() {
                self.fix_references_in_schema(schema_item);
            }
        }
        
        if let Some(one_of) = &mut schema.oneOf {
            for schema_item in one_of.iter_mut() {
                self.fix_references_in_schema(schema_item);
            }
        }
        
        // Fix references in not
        if let Some(not) = &mut schema.not {
            self.fix_references_in_schema(not);
        }
    }
    
    /// Write content to a file, splitting it into chunks if necessary
    fn write_chunked_file(&self, output_dir: &Path, base_filename: &str, content: &str, file_ext: &str) -> Result<()> {
        // If content is smaller than max file size, write it to a single file
        if content.len() <= self.max_file_size {
            let file_path = output_dir.join(format!("{}.{}", base_filename, file_ext));
            let mut file = File::create(&file_path)
                .context(format!("Failed to create file: {:?}", file_path))?;
            
            file.write_all(content.as_bytes())
                .context(format!("Failed to write to file: {:?}", file_path))?;
            
            info!("Generated file: {:?}", file_path);
            return Ok(());
        }
        
        // Otherwise, split content into chunks and write multiple files
        let mut chunk_number = 1;
        let mut start_idx = 0;
        
        // Create a directory for chunked files
        let chunked_dir = output_dir.join(format!("{}-split", base_filename));
        fs::create_dir_all(&chunked_dir)
            .context(format!("Failed to create directory for chunked files: {:?}", chunked_dir))?;
        
        // Create an index file
        let index_path = output_dir.join(format!("{}.{}", base_filename, file_ext));
        let mut index_content = format!("// This file is an index for the chunked {} files\n", file_ext);
        index_content.push_str(&format!("// The content has been split into multiple files due to its large size\n"));
        index_content.push_str(&format!("// See the '{}-split' directory for the actual content files\n", base_filename));
        
        let mut file = File::create(&index_path)
            .context(format!("Failed to create index file: {:?}", index_path))?;
        
        file.write_all(index_content.as_bytes())
            .context(format!("Failed to write to index file: {:?}", index_path))?;
        
        // Create individual chunk files
        while start_idx < content.len() {
            let end_idx = if start_idx + self.max_file_size >= content.len() {
                content.len()
            } else {
                // Try to find a good split point (e.g., a newline) near the max size
                let potential_end = start_idx + self.max_file_size;
                let search_range = potential_end.saturating_sub(1000)..std::cmp::min(potential_end + 1000, content.len());
                
                // Find the next newline after the potential end
                content[search_range.clone()].find('\n')
                    .map(|pos| search_range.start + pos + 1)
                    .unwrap_or(potential_end)
            };
            
            let chunk = &content[start_idx..end_idx];
            let chunk_path = chunked_dir.join(format!("{}_{}.{}", base_filename, chunk_number, file_ext));
            
            let mut chunk_file = File::create(&chunk_path)
                .context(format!("Failed to create chunk file: {:?}", chunk_path))?;
            
            chunk_file.write_all(chunk.as_bytes())
                .context(format!("Failed to write to chunk file: {:?}", chunk_path))?;
            
            info!("Generated chunk file {} of {}: {:?}", chunk_number, 
                  (content.len() + self.max_file_size - 1) / self.max_file_size, 
                  chunk_path);
            
            start_idx = end_idx;
            chunk_number += 1;
        }
        
        info!("Content was split into {} chunks", chunk_number - 1);
        Ok(())
    }
    
    /// Generate JSON output
    fn generate_json(&self, output_dir: &Path, openapi: &OpenAPI) -> Result<()> {
        // Serialize the OpenAPI document to JSON
        let json = serde_json::to_string_pretty(openapi)
            .context("Failed to serialize OpenAPI document to JSON")?;
        
        // Write the JSON to a file, splitting if necessary
        self.write_chunked_file(output_dir, "openapi", &json, "json")?;
        
        info!("Generated OpenAPI JSON output");
        Ok(())
    }
    
    /// Generate YAML output
    fn generate_yaml(&self, output_dir: &Path, openapi: &OpenAPI) -> Result<()> {
        // Serialize the OpenAPI document to YAML
        let yaml = serde_yaml::to_string(openapi)
            .context("Failed to serialize OpenAPI document to YAML")?;
        
        // Write the YAML to a file, splitting if necessary
        self.write_chunked_file(output_dir, "openapi", &yaml, "yaml")?;
        
        info!("Generated OpenAPI YAML output");
        Ok(())
    }
    
    /// Generate Go output (docs.go)
    fn generate_go(&self, output_dir: &Path, openapi: &OpenAPI) -> Result<()> {
        // Convert openapi to JSON string (no pretty print for docs.go)
        let json = serde_json::to_string(openapi)
            .context("Failed to serialize OpenAPI document to JSON")?;
        
        // Escape JSON for Go template
        let escaped_json = json.replace("\\", "\\\\").replace("\"", "\\\"");
        
        // Create docs.go file content
        let mut content = String::new();
        content.push_str("// Code generated by swaggo-rust; DO NOT EDIT.\n");
        content.push_str("package docs\n\n");
        content.push_str("import (\n");
        content.push_str("\t\"bytes\"\n");
        content.push_str("\t\"encoding/json\"\n");
        content.push_str("\t\"io/ioutil\"\n");
        content.push_str("\t\"os\"\n");
        content.push_str("\t\"path/filepath\"\n");
        content.push_str("\t\"strings\"\n");
        content.push_str("\t\"text/template\"\n\n");
        content.push_str("\t\"github.com/swaggo/swag\"\n");
        content.push_str(")\n\n");
        
        // Check if we need to split the JSON content
        if escaped_json.len() <= self.max_file_size {
            // Regular single file approach
            content.push_str(&format!("var doc = `{}`\n\n", escaped_json));
        } else {
            // For large docs, split into chunks and create a function to load them
            content.push_str("// loadDocChunks loads the chunked doc content from files\n");
            content.push_str("func loadDocChunks() string {\n");
            content.push_str("\t// Try to determine the docs directory\n");
            content.push_str("\texecutable, err := os.Executable()\n");
            content.push_str("\tif err != nil {\n");
            content.push_str("\t\tpanic(\"Failed to get executable path: \" + err.Error())\n");
            content.push_str("\t}\n\n");
            content.push_str("\texecDir := filepath.Dir(executable)\n");
            content.push_str("\tpossibleDirs := []string{\n");
            content.push_str("\t\tfilepath.Join(execDir, \"docs-split\"),\n");
            content.push_str("\t\t\"./docs-split\",\n");
            content.push_str("\t\t\"../docs-split\",\n");
            content.push_str("\t}\n\n");
            content.push_str("\tvar docChunks []string\n");
            content.push_str("\tvar chunkDir string\n\n");
            content.push_str("\t// Find the chunks directory\n");
            content.push_str("\tfor _, dir := range possibleDirs {\n");
            content.push_str("\t\tif _, err := os.Stat(dir); err == nil {\n");
            content.push_str("\t\t\tchunkDir = dir\n");
            content.push_str("\t\t\tbreak\n");
            content.push_str("\t\t}\n");
            content.push_str("\t}\n\n");
            content.push_str("\tif chunkDir == \"\" {\n");
            content.push_str("\t\tpanic(\"Could not find docs-split directory\")\n");
            content.push_str("\t}\n\n");
            content.push_str("\t// Read all chunk files and concatenate them\n");
            content.push_str("\tfiles, err := ioutil.ReadDir(chunkDir)\n");
            content.push_str("\tif err != nil {\n");
            content.push_str("\t\tpanic(\"Failed to read docs-split directory: \" + err.Error())\n");
            content.push_str("\t}\n\n");
            content.push_str("\t// Find and sort the chunk files\n");
            content.push_str("\tfor _, file := range files {\n");
            content.push_str("\t\tname := file.Name()\n");
            content.push_str("\t\tif strings.HasPrefix(name, \"openapi_\") && strings.HasSuffix(name, \".json\") {\n");
            content.push_str("\t\t\tfilePath := filepath.Join(chunkDir, name)\n");
            content.push_str("\t\t\tchunkData, err := ioutil.ReadFile(filePath)\n");
            content.push_str("\t\t\tif err != nil {\n");
            content.push_str("\t\t\t\tpanic(\"Failed to read chunk file: \" + err.Error())\n");
            content.push_str("\t\t\t}\n");
            content.push_str("\t\t\tdocChunks = append(docChunks, string(chunkData))\n");
            content.push_str("\t\t}\n");
            content.push_str("\t}\n\n");
            content.push_str("\t// Combine all chunks\n");
            content.push_str("\treturn strings.Join(docChunks, \"\")\n");
            content.push_str("}\n\n");
            
            // Use a function to load the doc content
            content.push_str("func getDoc() string {\n");
            content.push_str("\treturn loadDocChunks()\n");
            content.push_str("}\n\n");
        }
        
        // SwaggerInfo struct
        content.push_str("type swaggerInfo struct {\n");
        content.push_str("\tVersion     string\n");
        content.push_str("\tTitle       string\n");
        content.push_str("\tDescription string\n");
        
        // For OpenAPI 3, we use servers instead of host/basePath/schemes
        content.push_str("\tHost        string\n");
        content.push_str("\tBasePath    string\n");
        content.push_str("\tSchemes     []string\n");
        content.push_str("}\n\n");
        
        // SwaggerInfo variable
        content.push_str("// SwaggerInfo holds exported Swagger Info so clients can modify it\n");
        content.push_str("var SwaggerInfo = swaggerInfo{\n");
        content.push_str(&format!("\tVersion:     \"{}\",\n", openapi.info.version));
        
        // For OpenAPI 3, we derive host/basePath/schemes from the servers array
        let (host, base_path, schemes) = if let Some(servers) = &openapi.servers {
            if let Some(server) = servers.first() {
                if let Ok(url) = url::Url::parse(&server.url) {
                    let host = url.host_str().unwrap_or("").to_string();
                    let port = url.port().map(|p| format!(":{}", p)).unwrap_or_default();
                    let host_with_port = format!("{}{}", host, port);
                    let base_path = url.path().to_string();
                    let scheme = url.scheme().to_string();
                    (host_with_port, base_path, vec![scheme])
                } else {
                    (String::new(), String::new(), Vec::new())
                }
            } else {
                (String::new(), String::new(), Vec::new())
            }
        } else {
            (String::new(), String::new(), Vec::new())
        };
        
        content.push_str(&format!("\tHost:        \"{}\",\n", host));
        content.push_str(&format!("\tBasePath:    \"{}\",\n", base_path));
        
        // Schemes array
        content.push_str("\tSchemes:     []string{");
        let schemes_str = schemes.iter()
            .map(|s| format!("\"{}\"", s))
            .collect::<Vec<_>>()
            .join(", ");
        content.push_str(&schemes_str);
        content.push_str("},\n");
        
        content.push_str(&format!("\tTitle:       \"{}\",\n", openapi.info.title));
        
        // Description might contain newlines, escape them
        let description = openapi.info.description.as_deref().unwrap_or("")
            .replace("\n", "\\n");
        content.push_str(&format!("\tDescription: \"{}\",\n", description));
        content.push_str("}\n\n");
        
        // Reader struct and methods
        content.push_str("type s struct{}\n\n");
        content.push_str("func (s *s) ReadDoc() string {\n");
        content.push_str("\tsInfo := SwaggerInfo\n");
        content.push_str("\tsInfo.Description = strings.Replace(sInfo.Description, \"\\n\", \"\\\\n\", -1)\n\n");
        
        // Adjust the ReadDoc function based on whether we're using chunked files
        if escaped_json.len() <= self.max_file_size {
            content.push_str("\tt, err := template.New(\"swagger_info\").Funcs(template.FuncMap{\n");
            content.push_str("\t\t\"marshal\": func(v interface{}) string {\n");
            content.push_str("\t\t\ta, _ := json.Marshal(v)\n");
            content.push_str("\t\t\treturn string(a)\n");
            content.push_str("\t\t},\n");
            content.push_str("\t\t\"escape\": func(v string) string {\n");
            content.push_str("\t\t\t// escape backslashes\n");
            content.push_str("\t\t\tv = strings.Replace(v, \"\\\\\", \"\\\\\\\\\", -1)\n");
            content.push_str("\t\t\t// escape double quotes\n");
            content.push_str("\t\t\tv = strings.Replace(v, \"\\\"\", \"\\\\\\\"\", -1)\n");
            content.push_str("\t\t\treturn v\n");
            content.push_str("\t\t},\n");
            content.push_str("\t}).Parse(doc)\n");
        } else {
            content.push_str("\tdocStr := getDoc()\n");
            content.push_str("\tt, err := template.New(\"swagger_info\").Funcs(template.FuncMap{\n");
            content.push_str("\t\t\"marshal\": func(v interface{}) string {\n");
            content.push_str("\t\t\ta, _ := json.Marshal(v)\n");
            content.push_str("\t\t\treturn string(a)\n");
            content.push_str("\t\t},\n");
            content.push_str("\t\t\"escape\": func(v string) string {\n");
            content.push_str("\t\t\t// escape backslashes\n");
            content.push_str("\t\t\tv = strings.Replace(v, \"\\\\\", \"\\\\\\\\\", -1)\n");
            content.push_str("\t\t\t// escape double quotes\n");
            content.push_str("\t\t\tv = strings.Replace(v, \"\\\"\", \"\\\\\\\"\", -1)\n");
            content.push_str("\t\t\treturn v\n");
            content.push_str("\t\t},\n");
            content.push_str("\t}).Parse(docStr)\n");
        }
        
        content.push_str("\tif err != nil {\n");
        if escaped_json.len() <= self.max_file_size {
            content.push_str("\t\treturn doc\n");
        } else {
            content.push_str("\t\treturn docStr\n");
        }
        content.push_str("\t}\n\n");
        content.push_str("\tvar tpl bytes.Buffer\n");
        content.push_str("\tif err := t.Execute(&tpl, sInfo); err != nil {\n");
        if escaped_json.len() <= self.max_file_size {
            content.push_str("\t\treturn doc\n");
        } else {
            content.push_str("\t\treturn docStr\n");
        }
        content.push_str("\t}\n\n");
        content.push_str("\treturn tpl.String()\n");
        content.push_str("}\n\n");
        
        // Init function
        content.push_str("func init() {\n");
        content.push_str("\tswag.Register(\"swagger\", &s{})\n");
        content.push_str("}\n");
        
        // Write the content to a file
        let go_path = output_dir.join("docs.go");
        let mut file = File::create(&go_path)
            .context(format!("Failed to create file: {:?}", go_path))?;
        
        file.write_all(content.as_bytes())
            .context(format!("Failed to write to file: {:?}", go_path))?;
        
        // If we need to split the JSON, also write the chunked JSON files
        if escaped_json.len() > self.max_file_size {
            let chunked_dir = output_dir.join("docs-split");
            fs::create_dir_all(&chunked_dir)
                .context(format!("Failed to create directory for chunked files: {:?}", chunked_dir))?;
            
            // Write the JSON chunks
            self.write_chunked_file(&chunked_dir, "openapi", &json, "json")?;
        }
        
        info!("Generated Go file: {:?}", go_path);
        Ok(())
    }
    
    /// Generate Swagger UI HTML template for Go applications
    fn generate_swagger_ui(&self, output_dir: &Path) -> Result<()> {
        let html_path = output_dir.join("swagger-ui.html");
        
        let html_content = r###"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>API Documentation</title>
    <link rel="stylesheet" type="text/css" href="https://unpkg.com/swagger-ui-dist@5.21.0/swagger-ui.css">
    <style>
        body {
            margin: 0;
            padding: 0;
        }
        #swagger-ui {
            max-width: 1200px;
            margin: 0 auto;
        }
        .loading-indicator {
            text-align: center;
            padding: 20px;
            display: none;
        }
        .loading-indicator.visible {
            display: block;
        }
    </style>
</head>
<body>
    <div id="swagger-ui"></div>
    <div id="loading" class="loading-indicator">
        <h2>Loading API Documentation</h2>
        <p>If the documentation is large, this may take a moment...</p>
    </div>
    <script src="https://unpkg.com/swagger-ui-dist@5.21.0/swagger-ui-bundle.js"></script>
    <script>
        window.onload = function() {
            const loadingIndicator = document.getElementById('loading');
            loadingIndicator.classList.add('visible');
            
            // Function to check if we need to load chunked files
            async function checkForChunkedFiles() {
                try {
                    // First try to fetch the main openapi.json file
                    const response = await fetch('/docs/openapi.json');
                    const text = await response.text();
                    
                    // Check if the response is a chunked file index
                    if (text.includes('// This file is an index for the chunked json files')) {
                        console.log('Detected chunked JSON files, loading chunks...');
                        return await loadChunkedFiles();
                    } else {
                        // It's a regular file, return it directly
                        return JSON.parse(text);
                    }
                } catch (error) {
                    console.error('Error loading OpenAPI spec:', error);
                    throw error;
                }
            }
            
            // Function to load and combine chunked files
            async function loadChunkedFiles() {
                try {
                    // Get the list of chunk files
                    const response = await fetch('/docs/openapi-split/');
                    const files = await processDirectoryListing(response);
                    
                    // Sort the chunk files by index
                    const chunkFiles = files
                        .filter(file => file.startsWith('openapi_') && file.endsWith('.json'))
                        .sort((a, b) => {
                            // Extract chunk numbers for sorting
                            const numA = parseInt(a.replace('openapi_', '').replace('.json', ''));
                            const numB = parseInt(b.replace('openapi_', '').replace('.json', ''));
                            return numA - numB;
                        });
                    
                    if (chunkFiles.length === 0) {
                        throw new Error('No chunk files found');
                    }
                    
                    console.log(`Found ${chunkFiles.length} chunk files to load`);
                    
                    // Load each chunk and combine them
                    let combinedContent = '';
                    for (const file of chunkFiles) {
                        const chunkResponse = await fetch('/docs/openapi-split/' + file);
                        const chunkText = await chunkResponse.text();
                        combinedContent += chunkText;
                    }
                    
                    // Parse the combined content
                    return JSON.parse(combinedContent);
                } catch (error) {
                    console.error('Error loading chunked files:', error);
                    throw error;
                }
            }
            
            // Helper function to process directory listing
            function processDirectoryListing(response) {
                return response.text().then(text => {
                    // This is a simple parsing of directory listing HTML
                    // May need adjustment depending on the server's directory listing format
                    const regex = /href="([^"]+\.json)"/g;
                    const files = [];
                    let match;
                    while ((match = regex.exec(text)) !== null) {
                        files.push(match[1]);
                    }
                    return files;
                });
            }
            
            // Initialize Swagger UI with either the main file or combined chunks
            checkForChunkedFiles()
                .then(spec => {
                    loadingIndicator.classList.remove('visible');
                    
                    const ui = SwaggerUIBundle({
                        spec: spec,
                        dom_id: "#swagger-ui",
                        deepLinking: true,
                        presets: [
                            SwaggerUIBundle.presets.apis,
                            SwaggerUIBundle.SwaggerUIStandalonePreset
                        ],
                    });
                })
                .catch(error => {
                    // Fallback to standard URL loading if our chunk detection fails
                    console.log('Falling back to standard URL loading:', error);
                    loadingIndicator.classList.remove('visible');
                    
                    const ui = SwaggerUIBundle({
                        url: "/docs/openapi.json",
                        dom_id: "#swagger-ui",
                        deepLinking: true,
                        presets: [
                            SwaggerUIBundle.presets.apis,
                            SwaggerUIBundle.SwaggerUIStandalonePreset
                        ],
                    });
                });
        };
    </script>
</body>
</html>"###;

        let mut file = File::create(&html_path)
            .context(format!("Failed to create file: {:?}", html_path))?;
        
        file.write_all(html_content.as_bytes())
            .context(format!("Failed to write to file: {:?}", html_path))?;
        
        info!("Generated Swagger UI HTML: {:?}", html_path);
        Ok(())
    }

    /// Generate Go template for serving Swagger UI
    fn generate_swagger_handler(&self, output_dir: &Path) -> Result<()> {
        let handler_path = output_dir.join("swagger_handler.go");
        
        let handler_content = r###"// Code generated by swaggo-rust; DO NOT EDIT.
package docs

import (
	"net/http"
	"html/template"
	"path/filepath"
	"os"
	"strings"
)

// SwaggerUITemplate defines the HTML template for Swagger UI
const SwaggerUITemplate = `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>API Documentation</title>
    <link rel="stylesheet" type="text/css" href="https://unpkg.com/swagger-ui-dist@5.21.0/swagger-ui.css">
    <style>
        body {
            margin: 0;
            padding: 0;
        }
        #swagger-ui {
            max-width: 1200px;
            margin: 0 auto;
        }
        .loading-indicator {
            text-align: center;
            padding: 20px;
            display: none;
        }
        .loading-indicator.visible {
            display: block;
        }
    </style>
</head>
<body>
    <div id="swagger-ui"></div>
    <div id="loading" class="loading-indicator">
        <h2>Loading API Documentation</h2>
        <p>If the documentation is large, this may take a moment...</p>
    </div>
    <script src="https://unpkg.com/swagger-ui-dist@5.21.0/swagger-ui-bundle.js"></script>
    <script>
        window.onload = function() {
            const loadingIndicator = document.getElementById('loading');
            loadingIndicator.classList.add('visible');
            
            // Function to check if we need to load chunked files
            async function checkForChunkedFiles() {
                try {
                    // First try to fetch the main openapi.json file
                    const response = await fetch('/docs/openapi.json');
                    const text = await response.text();
                    
                    // Check if the response is a chunked file index
                    if (text.includes('// This file is an index for the chunked json files')) {
                        console.log('Detected chunked JSON files, loading chunks...');
                        return await loadChunkedFiles();
                    } else {
                        // It's a regular file, return it directly
                        return JSON.parse(text);
                    }
                } catch (error) {
                    console.error('Error loading OpenAPI spec:', error);
                    throw error;
                }
            }
            
            // Function to load and combine chunked files
            async function loadChunkedFiles() {
                try {
                    // Get the list of chunk files
                    const response = await fetch('/docs/openapi-split/');
                    const files = await processDirectoryListing(response);
                    
                    // Sort the chunk files by index
                    const chunkFiles = files
                        .filter(file => file.startsWith('openapi_') && file.endsWith('.json'))
                        .sort((a, b) => {
                            // Extract chunk numbers for sorting
                            const numA = parseInt(a.replace('openapi_', '').replace('.json', ''));
                            const numB = parseInt(b.replace('openapi_', '').replace('.json', ''));
                            return numA - numB;
                        });
                    
                    if (chunkFiles.length === 0) {
                        throw new Error('No chunk files found');
                    }
                    
                    console.log('Found ${chunkFiles.length} chunk files to load');
                    
                    // Load each chunk and combine them
                    let combinedContent = '';
                    for (const file of chunkFiles) {
                        const chunkResponse = await fetch('/docs/openapi-split/' + file);
                        const chunkText = await chunkResponse.text();
                        combinedContent += chunkText;
                    }
                    
                    // Parse the combined content
                    return JSON.parse(combinedContent);
                } catch (error) {
                    console.error('Error loading chunked files:', error);
                    throw error;
                }
            }
            
            // Helper function to process directory listing
            function processDirectoryListing(response) {
                return response.text().then(text => {
                    // This is a simple parsing of directory listing HTML
                    // May need adjustment depending on the server's directory listing format
                    const regex = /href="([^"]+\.json)"/g;
                    const files = [];
                    let match;
                    while ((match = regex.exec(text)) !== null) {
                        files.push(match[1]);
                    }
                    return files;
                });
            }
            
            // Initialize Swagger UI with either the main file or combined chunks
            checkForChunkedFiles()
                .then(spec => {
                    loadingIndicator.classList.remove('visible');
                    
                    const ui = SwaggerUIBundle({
                        spec: spec,
                        dom_id: "#swagger-ui",
                        deepLinking: true,
                        presets: [
                            SwaggerUIBundle.presets.apis,
                            SwaggerUIBundle.SwaggerUIStandalonePreset
                        ],
                    });
                })
                .catch(error => {
                    // Fallback to standard URL loading if our chunk detection fails
                    console.log('Falling back to standard URL loading:', error);
                    loadingIndicator.classList.remove('visible');
                    
                    const ui = SwaggerUIBundle({
                        url: "/docs/openapi.json",
                        dom_id: "#swagger-ui",
                        deepLinking: true,
                        presets: [
                            SwaggerUIBundle.presets.apis,
                            SwaggerUIBundle.SwaggerUIStandalonePreset
                        ],
                    });
                });
        };
    </script>
</body>
</html>`

// ServeSwaggerUI serves the Swagger UI HTML page
func ServeSwaggerUI(w http.ResponseWriter, r *http.Request) {
	tmpl, err := template.New("swagger-ui").Parse(SwaggerUITemplate)
	if err != nil {
		http.Error(w, "Failed to parse template", http.StatusInternalServerError)
		return
	}

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	err = tmpl.Execute(w, nil)
	if err != nil {
		http.Error(w, "Failed to render template", http.StatusInternalServerError)
		return
	}
}

// ServeOpenAPISpec serves the OpenAPI specification, handling chunked files if needed
func ServeOpenAPISpec(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	
	// Check if we're serving the main file or one of the chunks
	path := r.URL.Path
	if strings.HasSuffix(path, "openapi.json") {
		// Check if we have chunked files
		splitDir := filepath.Join("docs-split")
		if _, err := os.Stat(splitDir); err == nil {
			// We have a split directory, check if the index file exists
			indexContent, err := os.ReadFile(filepath.Join("docs", "openapi.json"))
			if err == nil && strings.Contains(string(indexContent), "chunked json files") {
				// This is an index file, serve it as is
				w.Write(indexContent)
				return
			}
		}
		
		// No chunks, serve the regular file
		http.ServeFile(w, r, filepath.Join("docs", "openapi.json"))
		return
	}
	
	// Handle requests for chunk files
	if strings.Contains(path, "openapi-split") {
		splitPath := strings.TrimPrefix(path, "/docs/openapi-split/")
		if splitPath == "" {
			// Directory listing requested, serve listing of available chunks
			ServeDirectoryListing(w, r, filepath.Join("docs-split"))
			return
		}
		
		// Serve the specific chunk file
		http.ServeFile(w, r, filepath.Join("docs-split", splitPath))
		return
	}
	
	// Any other path, return 404
	http.NotFound(w, r)
}

// ServeDirectoryListing generates a simple directory listing for the chunks
func ServeDirectoryListing(w http.ResponseWriter, r *http.Request, dir string) {
	files, err := os.ReadDir(dir)
	if err != nil {
		http.Error(w, "Failed to read directory", http.StatusInternalServerError)
		return
	}
	
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.Write([]byte("<html><body><ul>"))
	
	for _, file := range files {
		if !file.IsDir() && strings.HasSuffix(file.Name(), ".json") {
			w.Write([]byte("<li><a href=\"" + file.Name() + "\">" + file.Name() + "</a></li>"))
		}
	}
	
	w.Write([]byte("</ul></body></html>"))
}
"###;

        let mut file = File::create(&handler_path)
            .context(format!("Failed to create file: {:?}", handler_path))?;
        
        file.write_all(handler_content.as_bytes())
            .context(format!("Failed to write to file: {:?}", handler_path))?;
        
        info!("Generated Swagger UI handler: {:?}", handler_path);
        Ok(())
    }

    fn convert_to_openapi(&self) -> OpenAPI {
        let mut openapi = OpenAPI {
            openapi: self.openapi_version.clone(),
            info: self.api_info.info.clone(),
            paths: HashMap::new(),
            tags: self.api_info.tags.clone(),
            ..Default::default()
        };
        
        // Add servers
        if !self.api_info.servers.is_empty() {
            openapi.servers = Some(self.api_info.servers.clone());
        }
        
        // Add global security if any
        if !self.api_info.security.is_empty() {
            openapi.security = self.api_info.security.clone();
        }
        
        // Add external docs if any
        if let Some(ref docs) = self.api_info.external_docs {
            openapi.externalDocs = Some(docs.clone());
        }
        
        // Process operations
        for operation in &self.operations {
            let path = operation.path.clone();
            let method = operation.method.clone();
            
            // Ensure the path exists
            if !openapi.paths.contains_key(&path) {
                openapi.paths.insert(path.clone(), PathItem::default());
            }
            
            // Set the operation on the path item
            let path_item = openapi.paths.get_mut(&path).unwrap();
            match method.as_str() {
                "get" => path_item.get = Some(operation.operation.clone()),
                "post" => path_item.post = Some(operation.operation.clone()),
                "put" => path_item.put = Some(operation.operation.clone()),
                "delete" => path_item.delete = Some(operation.operation.clone()),
                "options" => path_item.options = Some(operation.operation.clone()),
                "head" => path_item.head = Some(operation.operation.clone()),
                "patch" => path_item.patch = Some(operation.operation.clone()),
                "trace" => path_item.trace = Some(operation.operation.clone()),
                _ => {}
            }
        }
        
        // Add components section with schemas
        if !self.schemas.is_empty() {
            let mut components = Components {
                schemas: self.schemas.clone(),
                responses: HashMap::new(),
                parameters: HashMap::new(),
                examples: HashMap::new(),
                requestBodies: HashMap::new(),
                headers: HashMap::new(),
                securitySchemes: self.api_info.security_definitions.clone(),
                links: HashMap::new(),
                callbacks: HashMap::new(),
                pathItems: HashMap::new(),
            };
            
            // Ensure schemas section exists
            components.ensure_schemas_exists();
            
            openapi.components = Some(components);
        } else if !self.api_info.security_definitions.is_empty() {
            // If we have security definitions but no schemas, still add the components section
            let mut components = Components {
                schemas: HashMap::new(),
                responses: HashMap::new(),
                parameters: HashMap::new(),
                examples: HashMap::new(),
                requestBodies: HashMap::new(),
                headers: HashMap::new(),
                securitySchemes: self.api_info.security_definitions.clone(),
                links: HashMap::new(),
                callbacks: HashMap::new(),
                pathItems: HashMap::new(),
            };
            
            // Ensure schemas section exists even if empty
            components.ensure_schemas_exists();
            
            openapi.components = Some(components);
        }
        
        // Ensure all referenced schemas are present in the document
        self.ensure_referenced_schemas_exist(&mut openapi);
        
        openapi
    }
    
    // Add method to ensure all referenced schemas exist in the components
    fn ensure_referenced_schemas_exist(&self, openapi: &mut OpenAPI) {
        // Create a function to find schema references in an object
        let mut references = HashSet::new();
        
        // Check paths
        for (_, path_item) in &openapi.paths {
            // Check each operation
            for operation in [&path_item.get, &path_item.post, &path_item.put, 
                              &path_item.delete, &path_item.options, &path_item.head, 
                              &path_item.patch, &path_item.trace].iter().filter_map(|&op| op.as_ref()) {
                
                // Check request body
                if let Some(request_body) = &operation.requestBody {
                    for (_, media_type) in &request_body.content {
                        if let Some(schema) = &media_type.schema {
                            self.collect_references(schema, &mut references);
                        }
                    }
                }
                
                // Check parameters
                for param in &operation.parameters {
                    if let Some(schema) = &param.schema {
                        self.collect_references(schema, &mut references);
                    }
                }
                
                // Check responses
                for (_, response) in &operation.responses {
                    for (_, media_type) in &response.content {
                        if let Some(schema) = &media_type.schema {
                            self.collect_references(schema, &mut references);
                        }
                    }
                }
            }
        }
        
        // Make sure all referenced schemas exist in components
        if let Some(components) = &mut openapi.components {
            for reference in references {
                // Extract the model name from the reference
                if let Some(model_name) = reference.strip_prefix("#/components/schemas/") {
                    if !components.schemas.contains_key(model_name) {
                        // If the schema doesn't exist but we have it in our schemas map
                        if let Some(schema) = self.schemas.get(model_name) {
                            components.schemas.insert(model_name.to_string(), schema.clone());
                        } else {
                            // If we don't have it, create a placeholder
                            components.schemas.insert(model_name.to_string(), Schema::default());
                        }
                    }
                }
            }
        }
    }
    
    // Helper to collect references from a schema
    fn collect_references(&self, schema: &Schema, references: &mut HashSet<String>) {
        if let Some(ref_) = &schema.ref_ {
            references.insert(ref_.clone());
        }
        
        if let Some(items) = &schema.items {
            self.collect_references(items, references);
        }
        
        if let Some(all_of) = &schema.allOf {
            for s in all_of {
                self.collect_references(s, references);
            }
        }
        
        if let Some(any_of) = &schema.anyOf {
            for s in any_of {
                self.collect_references(s, references);
            }
        }
        
        if let Some(one_of) = &schema.oneOf {
            for s in one_of {
                self.collect_references(s, references);
            }
        }
        
        if let Some(not) = &schema.not {
            self.collect_references(not, references);
        }
        
        for (_, prop) in &schema.properties {
            self.collect_references(prop, references);
        }
    }

    fn to_openapi(&self) -> OpenAPI {
        // Use the new implementation
        self.convert_to_openapi()
    }
} 