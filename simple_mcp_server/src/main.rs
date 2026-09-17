// Simple streamable HTTP MCP server, that lists files in current directory,
// or information about a file
//
// Helpful: https://dev.to/gde/build-an-mcp-server-in-rust-with-rmcp-a-walk-through-4cif

// MCP library imports (rmcp)
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ServerCapabilities, ServerConfig};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData, Json, ServerHandler, tool, tool_handler, tool_router};

// JSON imports (serde)
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
//use serde_json::Value;

// Server will be at /mcp on localhost:8080
const BIND_ADDRESS: &str = "127.0.0.1:8080";
const MCP_PATH: &str = "/mcp";

// Main function sets up and starts the server
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: why the Box?

    // Define the MCP service using rcmp (streamable HTTP service)
    let service = StreamableHttpService::new(
        || Ok(FileServer::new()),              // Our server, service factory
        LocalSessionManager::default().into(), // Tracks active client connections
        StreamableHttpServerConfig::default(), // Transport layer configuration
    );

    // Define axum router to put service under /mcp
    let router = axum::Router::new().nest_service(MCP_PATH, service);

    // Define tokio TCP listener
    let listener = tokio::net::TcpListener::bind(BIND_ADDRESS).await?;
    println!(
        "MCP server listening on http://{}{}",
        BIND_ADDRESS, MCP_PATH
    );

    // Start the axum server, listening on address and port, and using router to
    // send all requests to /mcp path
    axum::serve(listener, router).await?;

    Ok(())
}

// The following are schemas used for returning JSON results

// A single file, with information
#[derive(Serialize, Deserialize, JsonSchema)]
struct File {
    name: String,
    size: u32,
}

// A file name for which info has been requested
#[derive(Serialize, Deserialize, JsonSchema)]
struct FileRequest {
    name: String,
}

// MCP server exposing files in the data directory
#[derive(Clone)]
struct FileServer {
    tool_router: ToolRouter<Self>,
}

// The methods for the MCP server
#[tool_router(router = tool_router)]
impl FileServer {
    // Constructor
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    // Tool for listing files
    #[tool(description = "List the files in data directory")]
    fn list_files(&self) -> Result<Json<Vec<String>>, ErrorData> {
        let names = list_files()?;
        Ok(Json(names))
    }

    // Tool for getting information about one file, e.g., name, size, modification date
    #[tool(description = "Get information about a file, pass the file name")]
    fn get_file_info(&self, params: Parameters<FileRequest>) -> Result<Json<File>, ErrorData> {
        let info =
            file_info(params.0.name).map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        Ok(Json(info))
    }
}

// Enable tools for the MCP server, with description to allow the LLM to see what is available
#[tool_handler(router = self.tool_router)]
impl ServerHandler for FileServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(
                "Query file system. Use list_files to get list of files, and file_info to get information about a file.",
            )
    }
}

// TODO: get info about a file
fn file_info(filename: String) -> Result<File, ErrorData> {
    Ok(File {
        name: filename,
        size: 99,
    })
}

fn list_files() -> Result<Vec<String>, ErrorData> {
    Ok(vec![
        "file1".to_string(),
        "file2".to_string(),
        "file3".to_string(),
    ]) // TODO
}


/*
use chrono::{DateTime, Datelike, Utc};
use std::fs::{metadata, read_dir};
use std::io::Result;

// Structure for info about a file
#[derive(Debug)]
struct FileInfo {
    name: String,
    size: u64,
    modified: String,
    is_dir: bool,
}

// Get info for a file
fn file_info(filename: &str) -> Result<FileInfo> {
    let info = metadata(filename)?;

    // Convert file date to string
    let mdate = info.modified()?;
    let dt: DateTime<Utc> = mdate.into();
    let ymd = format!("{}-{:02}-{:02}", dt.year(), dt.month(), dt.day());

    Ok(FileInfo {
        name: filename.to_string(),
        size: info.len(),
        modified: ymd,
        is_dir: info.is_dir(),
    })
}

// Return a list of filenames in a directory
fn list_files(dir: &str) -> Result<Vec<String>> {
    Ok(read_dir(dir)?
        .map(|e| {
            e.unwrap()
                .path()
                .to_str() // necessary because filename is OsStr
                .unwrap()
                .to_string()
        })
        .collect())
}
*/

