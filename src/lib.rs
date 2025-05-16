pub mod generator;
pub mod models;
pub mod parser;

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    use crate::parser::GoParser;

    #[test]
    fn test_parse_general_api_info() {
        // Create a temporary directory
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("main.go");

        // Write a sample Go file with swagger annotations
        let mut file = File::create(&file_path).unwrap();
        write!(
            file,
            r#"
package main

// @title Swagger Example API
// @version 1.0
// @description This is a sample server.
// @termsOfService http://swagger.io/terms/

// @contact.name API Support
// @contact.url http://www.swagger.io/support
// @contact.email support@swagger.io

// @license.name Apache 2.0
// @license.url http://www.apache.org/licenses/LICENSE-2.0.html

// @host petstore.swagger.io
// @BasePath /v2
// @accept json
// @produce json
// @schemes http https

func main() {
    // ...
}
        "#
        )
        .unwrap();

        // Parse the API info
        let parser = GoParser::new();
        let api_info = parser.parse_general_api_info(&file_path).unwrap();

        // Verify the parsed info
        assert_eq!(api_info.info.title, "Swagger Example API");
        assert_eq!(api_info.info.version, "1.0");
        assert_eq!(
            api_info.info.description,
            Some("This is a sample server.".to_string())
        );
        assert_eq!(
            api_info.info.terms_of_service,
            Some("http://swagger.io/terms/".to_string())
        );

        assert!(api_info.info.contact.is_some());
        let contact = api_info.info.contact.unwrap();
        assert_eq!(contact.name, Some("API Support".to_string()));
        assert_eq!(
            contact.url,
            Some("http://www.swagger.io/support".to_string())
        );
        assert_eq!(contact.email, Some("support@swagger.io".to_string()));

        assert!(api_info.info.license.is_some());
        let license = api_info.info.license.unwrap();
        assert_eq!(license.name, "Apache 2.0");
        assert_eq!(
            license.url,
            Some("http://www.apache.org/licenses/LICENSE-2.0.html".to_string())
        );

        assert_eq!(api_info.host, Some("petstore.swagger.io".to_string()));
        assert_eq!(api_info.base_path, Some("/v2".to_string()));
        assert_eq!(api_info.consumes, vec!["application/json"]);
        assert_eq!(api_info.produces, vec!["application/json"]);
        assert_eq!(api_info.schemes, vec!["http", "https"]);
    }

    #[test]
    fn test_parse_operations() {
        // Create a temporary directory for test Go files
        let dir = tempdir().unwrap();

        // Create a test file
        let file_path = dir.path().join("main.go");
        let mut file = File::create(file_path).unwrap();

        // Write test content
        writeln!(file, "package main\n").unwrap();
        writeln!(file, "// @summary Test endpoint").unwrap();
        writeln!(file, "// @description Test description").unwrap();
        writeln!(file, "// @tags test").unwrap();
        writeln!(file, "// @produce application/json").unwrap();
        writeln!(file, "// @router /test [get]").unwrap();
        writeln!(file, "func test() {}").unwrap();

        // Parse operations
        let parser = crate::parser::GoParser::new();
        let operations = parser.parse_operations(&["."], dir.path()).unwrap();

        // Verify the parsed operations
        assert_eq!(operations.len(), 1);

        let op = operations.iter().next().unwrap();
        assert_eq!(op.path, "test");
        assert_eq!(op.operation.summary, Some("Test endpoint".to_string()));
        assert_eq!(
            op.operation.description,
            Some("Test description".to_string())
        );
        assert_eq!(op.operation.consumes, vec!["application/json"]);
        assert_eq!(op.operation.produces, vec!["application/json"]);
        assert_eq!(op.operation.parameters.len(), 0);
        assert!(op.operation.responses.contains_key("200"));
    }
}
