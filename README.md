# 🚀 Swaggo-Rust

![GitHub stars](https://img.shields.io/github/stars/budhilaw/swaggo-rust?style=social)
![License](https://img.shields.io/badge/license-MIT-blue)
![OpenAPI 3.1.1](https://img.shields.io/badge/OpenAPI-3.1.1-green)

> 📖 A powerful Rust implementation of [swaggo/swag](https://github.com/swaggo/swag) that generates OpenAPI documentation from Go code comments.

## ✨ Features

- 🔍 Parse Go code comments to extract Swagger/OpenAPI annotations
- 📊 Generate documentation in multiple formats (JSON, YAML, Go)
- 🌐 Generate Swagger UI templates and handlers for easy integration
- 🛠️ Support for all OpenAPI 3.1.1 features and annotations
- ⚡ Fast and memory-efficient implementation in Rust
- 📂 Support for splitting large output files into manageable chunks
- 🔒 Proper security scheme definitions and OAuth flows
- 🧩 Multiple server/host definitions for different environments
- 🚫 Exclude directories to prevent scanning unwanted code
- 🧠 Smart schema resolution to avoid reference errors

## 📥 Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/budhilaw/swaggo-rust.git
cd swaggo-rust

# Build the project
cargo build --release

# The binary will be available at ./target/release/swaggo-rust
```

### With Cargo

```bash
cargo install swaggo-rust
```

### From GitHub Releases

Visit the [Releases page](https://github.com/budhilaw/swaggo-rust/releases) to download pre-built binaries for your platform.

## 🚦 Quick Start

```bash
# Initialize swagger docs for a Go project
swaggo-rust init -g ./main.go -d ./controllers -o ./docs
```

## 📋 Command-line Reference

```
USAGE:
    swaggo-rust [OPTIONS] <SUBCOMMAND>

OPTIONS:
    -v, --verbose    Enable verbose output

SUBCOMMANDS:
    init    Initialize swagger documentation
    fmt     Format swagger comments
    help    Print this message or the help of the given subcommand(s)
```

### Init Subcommand

```
USAGE:
    swaggo-rust init [OPTIONS]

OPTIONS:
    -g, --general-info <GENERAL_INFO>    Go file path containing 'general API Info' [default: main.go]
    -d, --dir <DIR>                      Directories to parse, comma separated [default: ./]
    -o, --output <o>                     Output directory for generated files [default: ./docs]
    --ot <OUTPUT_TYPES>                  Output types to generate (go,json,yaml,ui) [default: go,json,yaml,ui]
    --oas <OPENAPI_VERSION>              OpenAPI version (3.0.0, 3.1.0, 3.1.1) [default: 3.1.1]
    --max-file-size <MAX_FILE_SIZE>      Maximum file size in MB before splitting files [default: 5]
    --exclude-dir <EXCLUDE_DIR>          Directories to exclude, comma separated
```

## 📝 Implementation Guide

### 1. General API Info (main.go)

Add these annotations to your main file:

```go
// @title My Amazing API
// @version 1.0
// @description This is a sample server showcasing Swaggo-Rust.
// @termsOfService https://myapi.com/terms/

// @contact.name API Support
// @contact.url https://myapi.com/support
// @contact.email support@myapi.com

// @license.name Apache 2.0
// @license.url http://www.apache.org/licenses/LICENSE-2.0.html

// @host api.myapp.com
// @BasePath /api/v1

// @securityDefinitions.apikey ApiKeyAuth
// @in header
// @name Authorization

func main() {
    // Your Go application code
}
```

### 2. Multiple Servers/Hosts (OpenAPI 3 Feature)

```go
// @server.url https://development-api.myapp.com
// @server.description Development server

// @server.url https://staging-api.myapp.com
// @server.description Staging server

// @server.url https://api.myapp.com
// @server.description Production server
```

### 3. Controller Implementation

In your controllers, add annotations for each endpoint:

```go
// UserController handles user-related endpoints
type UserController struct {
    // your dependencies
}

// GetUser godoc
// @Summary Get a user by ID
// @Description Retrieve a single user by their unique identifier
// @Tags users
// @Accept json
// @Produce json
// @Param id path string true "User ID"
// @Success 200 {object} models.User
// @Failure 400 {object} models.ErrorResponse "Invalid ID format"
// @Failure 404 {object} models.ErrorResponse "User not found"
// @Failure 500 {object} models.ErrorResponse "Server error"
// @Router /users/{id} [get]
// @Security ApiKeyAuth
func (c *UserController) GetUser(w http.ResponseWriter, r *http.Request) {
    // Your implementation
}

// CreateUser godoc
// @Summary Create a new user
// @Description Register a new user in the system
// @Tags users
// @Accept json
// @Produce json
// @Param user body models.UserCreateRequest true "User information"
// @Success 201 {object} models.User
// @Failure 400 {object} models.ErrorResponse "Invalid input"
// @Failure 500 {object} models.ErrorResponse "Server error"
// @Router /users [post]
// @Security ApiKeyAuth
func (c *UserController) CreateUser(w http.ResponseWriter, r *http.Request) {
    // Your implementation
}
```

### 4. Models with Examples

Use struct tags to provide examples in your models:

```go
type User struct {
    ID        string    `json:"id" example:"123e4567-e89b-12d3-a456-426614174000"`
    Username  string    `json:"username" example:"john_doe"`
    Email     string    `json:"email" example:"john@example.com"`
    CreatedAt time.Time `json:"created_at" example:"2023-01-01T00:00:00Z"`
}

type UserCreateRequest struct {
    Username string `json:"username" example:"john_doe"`
    Email    string `json:"email" example:"john@example.com"`
    Password string `json:"password" example:"securepassword123"`
}

type ErrorResponse struct {
    Code    int    `json:"code" example:"400"`
    Message string `json:"message" example:"Invalid request parameters"`
}
```

## 🔧 Advanced Usage

### Excluding Directories

To prevent scanning unwanted directories (like vendor, test, or mocks):

```bash
swaggo-rust init -g ./main.go -d ./internal/,./pkg/ --exclude-dir="vendor,mocks,.devenv" -o ./docs
```

The `--exclude-dir` flag is useful for:
- Skipping third-party package directories (vendor, node_modules)
- Excluding test files, fixtures, and mock implementations
- Preventing scanning of build artifacts or temporary directories
- Avoiding parsing of standard library packages

This helps keep your API documentation focused on your actual endpoints and prevents unwanted types from appearing in your schemas.

### Schema Definitions

Swaggo-rust automatically extracts and generates schema definitions for all models referenced in your API endpoints. This includes:

- Request and response body types
- Parameters and return types
- Nested models and their properties

The tool ensures proper schema resolution to avoid reference errors in the generated documentation. Each referenced type will have a corresponding schema definition in the components section of the OpenAPI document.

Example of a generated schema:

```json
"components": {
  "schemas": {
    "models.User": {
      "type": "object",
      "properties": {
        "id": {
          "type": "string",
          "example": "123e4567-e89b-12d3-a456-426614174000"
        },
        "username": {
          "type": "string",
          "example": "john_doe"
        }
      },
      "required": ["id", "username"]
    }
  }
}
```

### Large API Projects

For large API specifications, swaggo-rust can automatically split the output files:

```bash
# Set a custom max file size of 2MB
swaggo-rust init -g ./main.go -d ./,./controllers --max-file-size 2
```

### Integration in Go Applications

```go
import (
    "net/http"
    
    // Import the generated docs package
    "your_project/docs"
)

func main() {
    // Register routes
    http.HandleFunc("/api/v1/health", healthCheck)
    
    // Serve both regular and chunked OpenAPI files
    http.HandleFunc("/docs/openapi.json", docs.ServeOpenAPISpec)
    http.HandleFunc("/docs/openapi-split/", docs.ServeOpenAPISpec)
    
    // Serve Swagger UI with enhanced chunk loading
    http.HandleFunc("/swagger/", docs.ServeSwaggerUI)
    
    // Start server
    http.ListenAndServe(":8080", nil)
}
```

## 🔐 Authentication Examples

### Basic Auth

```go
// @securityDefinitions.basic BasicAuth
```

### API Key

```go
// @securityDefinitions.apikey ApiKeyAuth
// @in header
// @name Authorization
```

### JWT Bearer

```go
// @securityDefinitions.bearer JWTAuth
// @in header
// @name Authorization
// @description JWT Authorization header using the Bearer scheme
```

### OAuth2

```go
// @securityDefinitions.oauth2.implicit OAuth2Implicit
// @authorizationUrl https://example.com/oauth/authorize
// @scope.read Grants read access
// @scope.write Grants write access

// @securityDefinitions.oauth2.password OAuth2Password
// @tokenUrl https://example.com/oauth/token
// @scope.read Grants read access
// @scope.write Grants write access

// @securityDefinitions.oauth2.clientCredentials OAuth2ClientCredentials
// @tokenUrl https://example.com/oauth/token
// @scope.admin Grants admin access

// @securityDefinitions.oauth2.authorizationCode OAuth2AuthCode
// @authorizationUrl https://example.com/oauth/authorize
// @tokenUrl https://example.com/oauth/token
// @scope.read Grants read access
// @scope.write Grants write access
```

## 📊 Performance

Benchmarks show that swaggo-rust is significantly faster than the original Go implementation:

- 🚄 Parsing large projects: 2-3x faster
- 🧠 Memory usage: 30-40% less RAM

## 🔍 Troubleshooting

### Common Issues

1. **Unwanted Endpoints in Documentation**:
   - Use `--exclude-dir` to exclude directories containing test files or mocks
   - Make sure to only scan directories containing your API code

2. **Missing Endpoints**:
   - Check that your annotations follow the correct format
   - Ensure you're scanning all relevant directories with the `-d` flag

3. **Schema Reference Errors**:
   - These usually occur when referenced models aren't properly extracted
   - Ensure your models are properly defined with struct tags
   - Make sure the model package is included in the directories being scanned

4. **Large File Size**:
   - Adjust `--max-file-size` parameter to enable file splitting
   - Consider limiting the scope with more specific directory paths

## 📜 License

MIT

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

---

Made with ❤️ by [Budhilaw](https://github.com/budhilaw) 