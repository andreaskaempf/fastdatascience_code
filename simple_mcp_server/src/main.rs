use chrono::{DateTime, Datelike, Utc};
use std::fs::{metadata, read_dir};
use std::io::Result;

fn main() -> Result<()> {
    for f in list_files("/tmp")? {
        let info = file_info(&f)?;
        println!("{:?}", info);
    }
    Ok(())
}

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
