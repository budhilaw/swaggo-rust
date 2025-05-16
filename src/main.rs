use anyhow::Result;
use clap::{Parser, Subcommand};
use env_logger::Env;
use log::{debug, info};
use std::path::Path;
use walkdir::WalkDir;

mod parser;
mod generator;
mod models;

/// Rust implementation of swaggo/swag for generating OpenAPI 3.1.1 documents from Go annotations
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize OpenAPI documentation
    Init {
        /// Go file path in which 'general API Info' is written
        #[arg(short = 'g', long)]
        general_info: Option<String>,

        /// Directories to parse, comma separated
        #[arg(short, long, default_value = "./")]
        dir: String,

        /// Output directory for generated files
        #[arg(short, long, default_value = "./docs")]
        output: String,

        /// Output types to generate (go,json,yaml,ui)
        #[arg(long = "ot", default_value = "go,json,yaml,ui")]
        output_types: String,
        
        /// OpenAPI version (3.0.0, 3.1.0, 3.1.1)
        #[arg(long = "oas", default_value = "3.1.1")]
        openapi_version: String,

        /// Maximum file size in MB before splitting files (default: 5)
        #[arg(long = "max-file-size", default_value = "5")]
        max_file_size: usize,
        
        /// Directories to exclude, comma separated
        #[arg(long = "exclude-dir")]
        exclude_dir: Option<String>,
    },

    /// Format OpenAPI comments
    Fmt {
        /// Directories to parse, comma separated
        #[arg(short, long, default_value = "./")]
        dir: String,

        /// Go file path in which 'general API Info' is written
        #[arg(short = 'g', long)]
        general_info: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize logger with appropriate verbosity level
    let env = Env::default().filter_or("RUST_LOG", if cli.verbose { "debug" } else { "info" });
    env_logger::init_from_env(env);
    
    debug!("Starting swaggo-rust...");
    
    match &cli.command {
        Commands::Init { general_info, dir, output, output_types, openapi_version, max_file_size, exclude_dir } => {
            info!("Initializing OpenAPI docs");
            
            let dirs: Vec<String> = dir.split(',').map(|s| s.trim().to_string()).collect();
            let output_types: Vec<String> = output_types.split(',').map(|s| s.trim().to_string()).collect();
            let excluded_dirs: Vec<String> = exclude_dir
                .as_ref()
                .map(|ed| ed.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default();
            
            // Convert max_file_size from MB to bytes
            let max_file_size_bytes = max_file_size * 1024 * 1024;
            debug!("Maximum file size: {} MB ({} bytes)", max_file_size, max_file_size_bytes);
            
            // Create the parser
            let parser = parser::GoParser::new();
            
            // Find the general API info file if not provided
            let found_general_info = match general_info {
                Some(path) => path.clone(),
                None => find_general_api_info_file(&dirs)?,
            };
            
            debug!("General info file: {}", found_general_info);
            debug!("Directories to parse: {}", dir);
            if !excluded_dirs.is_empty() {
                debug!("Directories to exclude: {}", exclude_dir.as_ref().unwrap());
            }
            debug!("Output directory: {}", output);
            debug!("OpenAPI version: {}", openapi_version);
            
            // Parse the API info
            let api_info = parser.parse_general_api_info(&found_general_info)?;
            debug!("Parsed API info: {:?}", api_info);
            
            // Get the base directory for resolving imports
            // First try to find the directory containing go.mod
            let base_dir = {
                let found_general_info_path = Path::new(&found_general_info);
                let general_info_dir = found_general_info_path.parent().unwrap_or(Path::new("."));
                
                let mut current_dir = general_info_dir.to_path_buf();
                let mut go_mod_dir = current_dir.clone();
                
                // Try to locate go.mod file by walking up directories
                while current_dir.parent().is_some() {
                    let go_mod = current_dir.join("go.mod");
                    if go_mod.exists() {
                        go_mod_dir = current_dir.clone();
                        break;
                    }
                    current_dir = current_dir.parent().unwrap().to_path_buf();
                }
                
                go_mod_dir
            };
            
            debug!("Using base directory for imports: {:?}", base_dir);
            
            // Parse the API operations from all specified directories
            let (operations, schemas) = parser.parse_operations(&dirs, &excluded_dirs, &base_dir)?;
            debug!("Parsed {} operations", operations.len());
            debug!("Parsed {} schema definitions", schemas.len());
            
            // Generate the OpenAPI documentation
            let generator = generator::Generator::new_with_max_file_size(
                api_info, 
                operations, 
                schemas, 
                max_file_size_bytes, 
                openapi_version.to_string()
            );
            generator.generate(output, &output_types)?;
            
            info!("OpenAPI documentation generated successfully");
        },
        Commands::Fmt { general_info, dir } => {
            info!("Formatting OpenAPI comments");
            
            let dirs: Vec<String> = dir.split(',').map(|s| s.trim().to_string()).collect();
            
            // Find the general API info file if not provided
            let found_general_info = match general_info {
                Some(path) => path.clone(),
                None => find_general_api_info_file(&dirs)?,
            };
            
            debug!("General info file: {}", found_general_info);
            debug!("Directories to format: {}", dir);
            
            // TODO: Implement formatting
            info!("Formatting is not yet implemented");
        },
    }
    
    Ok(())
}

/// Finds a file containing general API info by searching common main.go files
fn find_general_api_info_file(dirs: &[String]) -> Result<String> {
    // Common locations for main.go or similar files
    let possible_files = vec![
        "main.go",
        "api/main.go",
        "cmd/main.go",
        "cmd/api/main.go",
        "cmd/server/main.go",
        "internal/main.go",
        "pkg/main.go",
    ];
    
    // First check in the provided directories
    for dir in dirs {
        for entry in WalkDir::new(dir).max_depth(3).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() && path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default() == "main.go" {
                debug!("Found potential general API info file: {:?}", path);
                return Ok(path.to_string_lossy().to_string());
            }
        }
    }
    
    // If not found in provided directories, check common locations
    for file_path in possible_files {
        if Path::new(file_path).exists() {
            debug!("Found potential general API info file: {}", file_path);
            return Ok(file_path.to_string());
        }
    }
    
    // If still not found, use the first directory and assume main.go
    debug!("No main.go found, defaulting to ./main.go");
    Ok("./main.go".to_string())
} 