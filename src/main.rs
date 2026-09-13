// Execute query on command line against Parquet files in the data directory

use datafusion::prelude::*;
use std::env;

#[tokio::main]
async fn main() -> datafusion::error::Result<()> {

    // Get query from command line
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Missing query");
        return Ok(());
    }
    let q = &args[1];
    println!("Query: {}", q);

    // Register the table
    let ctx = SessionContext::new();
    ctx.register_parquet("taxi", "data", ParquetReadOptions::new()).await?;

    // Create a plan to run a SQL query
    let df = ctx.sql(q).await?;

    // Execute and print results
    df.show().await?;
    Ok(())
}
