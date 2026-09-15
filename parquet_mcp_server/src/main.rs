// Streaming (streamable HTTP) MCP server, executes query against 
// Parquet files in the data directory

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{tool, tool_handler, tool_router, ErrorData, Json, ServerHandler};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

mod parquet;

// Server will be at /mcp on localhost:8080
const BIND_ADDRESS: &str = "127.0.0.1:8080";
const MCP_PATH: &str = "/mcp";

// Main function sets up and starts the server
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    // Define the MCP service using rcmp (streamable HTTP service)
    let service = StreamableHttpService::new(
        || Ok(ParquetServer::new()),            // Our server, service factory
        LocalSessionManager::default().into(),  // Tracks active client connections
        StreamableHttpServerConfig::default(),  // Transport layer configuration
    );

    // Define axum router to put service under /mcp
    let router = axum::Router::new().nest_service(MCP_PATH, service);

    // Define tokio TCP listener
    let listener = tokio::net::TcpListener::bind(BIND_ADDRESS).await?;
    println!("MCP server listening on http://{}{}", BIND_ADDRESS, MCP_PATH);

    // Start the axum server, listening on address and port, and using router to send all requests
    // to /mcp path
    axum::serve(listener, router).await?;

    Ok(())
}

// The following are schemas used for returning JSON results

// A table available to be queried
#[derive(Serialize, Deserialize, JsonSchema)]
struct Table {
    name: String,
}

// A single column of a table, with data type
#[derive(Serialize, Deserialize, JsonSchema)]
struct Field {
    name: String,
    data_type: String,
    nullable: bool,
}

// A table name for which list of fields has been requested
#[derive(Serialize, Deserialize, JsonSchema)]
struct FieldsRequest {
    table: String,
}

// SQL query to execute against the Parquet data
#[derive(Serialize, Deserialize, JsonSchema)]
struct QueryRequest {
    sql: String,
}

// MCP server exposing the Parquet data in the data directory
#[derive(Clone)]
struct ParquetServer {
    tool_router: ToolRouter<Self>,
}

// The methods for the MCP server
#[tool_router(router = tool_router)]
impl ParquetServer {

    // Constructor
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    // Tool for listing tables, read from the Parquet files in the data directory
    #[tool(description = "List the tables that can be queried")]
    async fn list_tables(&self) -> Result<Json<Vec<Table>>, ErrorData> {
        let names = parquet::list_tables()
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(
            names.into_iter().map(|name| Table { name }).collect(),
        ))
    }

    // Tool for listing columns of a table, read from the table's own schema
    #[tool(description = "List the fields (columns) of a table")]
    async fn list_fields(
        &self,
        params: Parameters<FieldsRequest>,
    ) -> Result<Json<Vec<Field>>, ErrorData> {
        let schema = parquet::table_schema(&params.0.table)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(
            schema
                .fields()
                .iter()
                .map(|f| Field {
                    name: f.name().clone(),
                    data_type: f.data_type().to_string(),
                    nullable: f.is_nullable(),
                })
                .collect(),
        ))
    }

    // Tool for executing SQL query and returning result, run with DataFusion
    #[tool(description = "Execute a SQL query and return the rows as JSON objects")]
    async fn query(&self, params: Parameters<QueryRequest>) -> Result<Json<Vec<Value>>, ErrorData> {
        let rows = parquet::query_json(&params.0.sql)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(rows))
    }
}

// Enable tools for the MCP server, with description to allow the LLM to see what is available
#[tool_handler(router = self.tool_router)]
impl ServerHandler for ParquetServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(
                "Query Parquet files with SQL. Use list_tables to see what is available, \
                 list_fields to see a table's columns, and query to run a SQL statement.",
            )
    }
}
