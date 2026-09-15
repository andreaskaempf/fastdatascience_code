// Execute query on command line against Parquet files in the data directory,
// which is assumed to be under the parent directory.
//
// Sample usage:  cargo run "select avg(fare_amount) from rides"
//
// Sample output:
// Query: select avg(fare_amount) from rides
// +------------------------+
// | avg(rides.fare_amount) |
// +------------------------+
// | 19.219927893175566     |
// +------------------------+

use datafusion::arrow::array::RecordBatch;
use datafusion::arrow::util::pretty::pretty_format_batches;
use datafusion::error::Result;
use datafusion::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Get query from the command line
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 2 {
        println!("Missing query or extra arguments");
        std::process::exit(1);
    }
    let q = args[1].as_str();
    println!("Query: {}", q);

    // Execute query and show result
    let batches = query(&q).await?;
    println!("{}", pretty_format_batches(&batches)?);
    Ok(())
}

// Execute a query, returning the resulting record batches
async fn query(q: &str) -> Result<Vec<RecordBatch>> {
    // Register the table
    let ctx = SessionContext::new();
    ctx.register_parquet("rides", "../data", ParquetReadOptions::new())
        .await?;

    // Create a plan to run a SQL query
    let df = ctx.sql(q).await?;

    // Execute and collect the results
    let batches = df.collect().await?;

    Ok(batches)
}
