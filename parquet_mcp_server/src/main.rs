// MCP server for serving Parquet data, has three commands:
//

// MCP library imports (rmcp)
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ServerCapabilities, ServerConfig};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData, Json, ServerHandler, tool, tool_handler, tool_router};

// DataFusion imports
use datafusion::arrow::json::writer::{JsonArray, WriterBuilder};
use datafusion::error::{DataFusionError, Result};
use datafusion::prelude::*;

// For JSON serialization
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

//------------------------------------------------------------------//
//                     BUILD AND LAUNCH SERVER                      //
//------------------------------------------------------------------//

// Server will be at /mcp on localhost:8080
const BIND_ADDRESS: &str = "127.0.0.1:8080";
const MCP_PATH: &str = "/mcp";

// Main function sets up and starts the server
#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    // Define the MCP service using rcmp (streamable HTTP service)
    let service = StreamableHttpService::new(
        || Ok(ParquetServer::new()),           // Our server, service factory
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

//------------------------------------------------------------------//
//                            SCHEMAS                               //
//------------------------------------------------------------------//

// Request schema for SQL query
#[derive(Serialize, Deserialize, JsonSchema)]
struct SchemaRequest {
    table: String,
}

// Information about one column of a table, used in SchemaResult
#[derive(Serialize, Deserialize, JsonSchema)]
struct ColumnInfo {
    name: String,
    data_type: String,
    nullable: bool,
}

// Schema of a table, i.e. its columns
#[derive(Serialize, Deserialize, JsonSchema)]
struct SchemaResult {
    table: String,
    columns: Vec<ColumnInfo>,
}

// Request schema for SQL query
#[derive(Serialize, Deserialize, JsonSchema)]
struct QueryRequest {
    sql: String,
}

// Information returned from a query, a list of rows
#[derive(Serialize, Deserialize, JsonSchema)]
struct QueryResult {
    records: Vec<Map<String, Value>>, // each row is a JSON object, column name -> value
}

//------------------------------------------------------------------//
//                         TOOL DEFINITIONS                         //
//------------------------------------------------------------------//

// MCP server exposing files in the data directory
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

    // Tool for listing available tables
    #[tool(description = "List all the files in data directory")]
    async fn tables(&self) -> Result<Json<Vec<String>>, ErrorData> {
        let tt = get_table_list()
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        Ok(Json(tt))
    }

    // Tool for getting the schema of a table
    #[tool(description = "Get the schema of a table")]
    async fn schema(
        &self,
        params: Parameters<SchemaRequest>,
    ) -> Result<Json<SchemaResult>, ErrorData> {
        let schema = get_table_schema(&params.0.table)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        Ok(Json(schema))
    }

    // Tool for running a query on a table
    #[tool(description = "Run a query on a table")]
    async fn query(
        &self,
        params: Parameters<QueryRequest>,
    ) -> Result<Json<QueryResult>, ErrorData> {
        let result = run_query(&params.0.sql)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        Ok(Json(result))
    }
}

// Enable tools for the MCP server, with description to allow the LLM to see what is available
#[tool_handler(router = self.tool_router)]
impl ServerHandler for ParquetServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("Query Parquet files, use tables to get list of tables, schema to get the schema for one table, and query to run an SQL query.")
    }
}

//------------------------------------------------------------------//
//                   DATAFUSION PARQUET FUNCTIONS                   //
//------------------------------------------------------------------//

// Get the Parquet session, using the data directory, and assigning it to a table.
// Note that this is a bit inefficient, as the context gets recreated each time,
// would be faster in production to have it reused (e.g., with tokio's OnceCell).
async fn session() -> Result<SessionContext> {
    let ctx = SessionContext::new();
    ctx.register_parquet("rides", "../data", ParquetReadOptions::new())
        .await?;
    Ok(ctx)
}

// List the names of the tables available to be queried
async fn get_table_list() -> Result<Vec<String>> {

    // Walk the schemas of each catalog, collecting the tables they contain
    let ctx = session().await?;
    let mut tables = Vec::new();
    for catalog_name in ctx.catalog_names() {
        // each catalog
        let Some(catalog) = ctx.catalog(&catalog_name) else {
            continue;
        };
        for schema_name in catalog.schema_names() {
            // each schema within the catalog
            let Some(schema) = catalog.schema(&schema_name) else {
                continue;
            };
            tables.extend(schema.table_names()); // add to list of tables
        }
    }
    tables.sort();

    Ok(tables)
}

// Get the schema of one table, i.e. its columns and their data types
async fn get_table_schema(table: &str) -> Result<SchemaResult> {

    // Get context and provider for this table (will fail if not there)
    let ctx = session().await?;
    let provider = ctx.table_provider(table).await?;

    // Get information on all the columns in this table, into a list of ColumnInfo structs,
    // so they can be serialized (raw return value from provider.schema() cannot be serialized)
    let columns = provider
        .schema()
        .fields()
        .iter()
        .map(|field| ColumnInfo {
            name: field.name().clone(),
            data_type: field.data_type().to_string(),
            nullable: field.is_nullable(),
        })
        .collect();

    // Return as a SchemaResult structure, defined above
    Ok(SchemaResult {
        table: table.to_string(),
        columns,
    })
}

// Execute a query, returning one JSON object per row of the result
async fn run_query(q: &str) -> Result<QueryResult> {

    // Create a plan to run a SQL query
    let ctx = session().await?;
    let df = ctx.sql(q).await?;

    // Execute and collect the results
    // TODO: this might be prohibitively huge, enforce limit?
    let batches = df.collect().await?;

    // Write the batches out as a JSON array, one object per row, so they
    // can be serialized (raw record batches cannot be serialized)
    let mut buffer = Vec::new();
    let mut writer = WriterBuilder::new() // DataFusion feature
        .with_explicit_nulls(true)
        .build::<_, JsonArray>(&mut buffer);
    writer.write_batches(&batches.iter().collect::<Vec<_>>())?;
    writer.finish()?;

    // Read the array back in as rows, and return as a QueryResult structure, defined above
    let records =
        serde_json::from_slice(&buffer).map_err(|e| DataFusionError::External(Box::new(e)))?;
    Ok(QueryResult { records })
}
