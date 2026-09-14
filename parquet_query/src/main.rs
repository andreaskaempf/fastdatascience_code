// Execute query on command line against Parquet files in the data directory

use datafusion::arrow::array::RecordBatch;
use datafusion::arrow::util::pretty::pretty_format_batches;
use datafusion::error::Result;
use datafusion::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {

    // Get query from the command line
    let mut args = std::env::args();
    let program = args.next().unwrap_or_else(|| "parquet_query".to_string());
    let Some(q) = args.next() else {
        eprintln!("Usage: {} \"<query>\"", program);
        std::process::exit(1);
    };

    let batches = query(&q).await?;
    println!("{}", pretty_format_batches(&batches)?);
    Ok(())
}

// Execute a query, returning the resulting record batches
async fn query(q: &str) -> Result<Vec<RecordBatch>> {
    // Register the table
    let ctx = SessionContext::new();
    ctx.register_parquet("taxi", "data", ParquetReadOptions::new())
        .await?;

    // Create a plan to run a SQL query
    let df = ctx.sql(q).await?;

    // Execute and collect the results
    let batches = df.collect().await?;

    Ok(batches)
}
