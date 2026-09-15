// Functions to query Parquet files, assumed to be in 'data' under the parent directory.

use datafusion::arrow::array::RecordBatch;
use datafusion::arrow::datatypes::SchemaRef;
use datafusion::arrow::json::writer::{JsonArray, WriterBuilder};
use datafusion::error::{DataFusionError, Result};
use datafusion::prelude::*;

use serde_json::Value;
use tokio::sync::OnceCell;

// The session is created once and then reused, so that the Parquet files are only
// registered, and their metadata read, on the first request that needs them
static SESSION: OnceCell<SessionContext> = OnceCell::const_new();

// Get the shared session, with the Parquet data registered as tables, creating it on
// first use (cloning a session is then cheap, as the clone shares the underlying state).
async fn session() -> Result<SessionContext> {
    let ctx = SESSION
        .get_or_try_init(|| async {
            let ctx = SessionContext::new();
            ctx.register_parquet("rides", "../data", ParquetReadOptions::new())
                .await?;

            Ok::<_, DataFusionError>(ctx)
        })
        .await?;

    Ok(ctx.clone())   // return a clone of the session
}

// List the names of the tables available to be queried
pub async fn list_tables() -> Result<Vec<String>> {
    let ctx = session().await?;

    // Walk the schemas of each catalog, collecting the tables they contain
    let mut tables = Vec::new();
    for catalog_name in ctx.catalog_names() {   // each catalog
        let Some(catalog) = ctx.catalog(&catalog_name) else {
            continue;
        };
        for schema_name in catalog.schema_names() {  // each schema within the catalog
            let Some(schema) = catalog.schema(&schema_name) else {
                continue;
            };
            tables.extend(schema.table_names());   // add to list of tables
        }
    }
    tables.sort();

    Ok(tables)
}

// Get the schema of one table, i.e. its columns and their data types
pub async fn table_schema(table: &str) -> Result<SchemaRef> {
    let ctx = session().await?;

    // Look up the table, creates an error if it is not registered
    let provider = ctx.table_provider(table).await?;

    Ok(provider.schema())
}

// Execute a query, returning the resulting record batches,
// used by query_json below
// TODO: can we merge this with query_json?
async fn query(q: &str) -> Result<Vec<RecordBatch>> {
    let ctx = session().await?;

    // Create a plan to run a SQL query
    let df = ctx.sql(q).await?;

    // Execute and collect the results
    // TODO: this might be prohibitively huge, enforce limit?
    let batches = df.collect().await?;  // TODO: is it necessary to collect here?

    Ok(batches)
}

// Execute a query, returning one JSON object per row of the result
pub async fn query_json(q: &str) -> Result<Vec<Value>> {
    let batches = query(q).await?;

    // Write the batches out as a JSON array, one object per row. Null columns are
    // written explicitly, so that every row has the same keys as the others.
    let mut buffer = Vec::new();
    let mut writer = WriterBuilder::new()  // DataFusion feature
        .with_explicit_nulls(true)
        .build::<_, JsonArray>(&mut buffer);
    writer.write_batches(&batches.iter().collect::<Vec<_>>())?;
    writer.finish()?;

    // Read the array back in as values we can hand to the caller
    serde_json::from_slice(&buffer).map_err(|e| DataFusionError::External(Box::new(e)))
}
