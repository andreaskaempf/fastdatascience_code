// Streaming (streamable HTTP) MCP server, executes query against 
// Parquet files in the data directory

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{tool, tool_handler, tool_router, Json, ServerHandler};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// Server will be at /mcp on localhost:8080
const BIND_ADDRESS: &str = "127.0.0.1:8080";
const MCP_PATH: &str = "/mcp";

// Main function starts the server
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    // Define rcmp streamable HTTP service
    let service = StreamableHttpService::new(
        || Ok(ParquetServer::new()),  // Our server, service factory
        LocalSessionManager::default().into(), // Tracks active client connections
        StreamableHttpServerConfig::default(),  // Transport layer configuration
    );

    // Define axum router to put service under /mcp
    let router = axum::Router::new().nest_service(MCP_PATH, service);

    // TCP listner
    let listener = tokio::net::TcpListener::bind(BIND_ADDRESS).await?;
    println!("MCP server listening on http://{}{}", BIND_ADDRESS, MCP_PATH);

    // Start the axum server, listening on address and port, and using router
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;

    Ok(())
}

// The following are schemas used for returning JSON results

// A table available to be queried
#[derive(Serialize, Deserialize, JsonSchema)]
struct Table {
    name: String,
    description: String,
}

// A single column of a table, with data type
#[derive(Serialize, Deserialize, JsonSchema)]
struct Field {
    name: String,
    data_type: String,
    nullable: bool,
}

// A field requested, for which we should return the schema
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

    // Tool for listing tables
    // TODO: read the table names from the Parquet files in the data directory
    #[tool(description = "List the tables that can be queried")]
    async fn list_tables(&self) -> Json<Vec<Table>> {
        Json(vec![Table {
            name: "taxi".to_string(),
            description: "Yellow taxi trip records".to_string(),
        }])
    }

    // Tool for listing columns of a table
    // TODO: read the schema of the requested table instead of returning a fixed one
    #[tool(description = "List the fields (columns) of a table")]
    async fn list_fields(&self, params: Parameters<FieldsRequest>) -> Json<Vec<Field>> {
        let _ = params.0.table;
        Json(vec![
            Field {
                name: "trip_id".to_string(),
                data_type: "BIGINT".to_string(),
                nullable: false,
            },
            Field {
                name: "pickup_at".to_string(),
                data_type: "TIMESTAMP".to_string(),
                nullable: false,
            },
            Field {
                name: "passenger_count".to_string(),
                data_type: "INTEGER".to_string(),
                nullable: true,
            },
            Field {
                name: "trip_distance".to_string(),
                data_type: "DOUBLE".to_string(),
                nullable: true,
            },
            Field {
                name: "total_amount".to_string(),
                data_type: "DOUBLE".to_string(),
                nullable: true,
            },
        ])
    }

    // Tool for executing SQL query and returning result
    // TODO: run the query with DataFusion and convert the record batches to JSON
    #[tool(description = "Execute a SQL query and return the rows as JSON objects")]
    async fn query(&self, params: Parameters<QueryRequest>) -> Json<Vec<Value>> {
        let _ = params.0.sql;
        Json(vec![
            json!({
                "trip_id": 1,
                "pickup_at": "2024-01-01T08:14:00",
                "passenger_count": 1,
                "trip_distance": 2.7,
                "total_amount": 14.30,
            }),
            json!({
                "trip_id": 2,
                "pickup_at": "2024-01-01T08:22:00",
                "passenger_count": 3,
                "trip_distance": 8.1,
                "total_amount": 36.75,
            }),
            json!({
                "trip_id": 3,
                "pickup_at": "2024-01-01T09:05:00",
                "passenger_count": 2,
                "trip_distance": 1.2,
                "total_amount": 9.55,
            }),
        ])
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
