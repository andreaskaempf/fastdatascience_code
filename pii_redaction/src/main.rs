// Read JSON file of German texts with PII, and redact the following:
// - IBAN bank numbers
// - Dates and times
// - Steuernummer tax IDs
// - IP addresses (IPv4 and IPv6)
// - Social security numbers
// - Phone numbers
// - Email addresses
// - Postal addresses
// - Driver's license numbers (Führerschein)
// - ID card numbers (Personalausweis)
// - First and last names, and titles such as "Herr" or "Dr."
//
// The data file is taken from https://huggingface.co/datasets/ai4privacy/pii-masking-300k/tree/main/data/train,
// use the script split_json.py to create a simpler JSON file with just the source and target text.

use std::fs::File;
use std::io::{BufRead, BufReader, Result};

use serde::{Deserialize, Serialize}; // do NOT use the Result provided by Serde

// Redaction function in separate module, with name detection in its own module
mod names;
mod redact;
use crate::redact::redact;

fn main() -> Result<()> {
    // Read input file
    let file = File::open("data/texts.json")?;
    let reader = BufReader::new(file);

    // Process each line
    let mut n = 0;
    let mut matches = 0;
    for line in reader.lines() {
        let ok = process_line(line?.as_str())?;
        if ok {
            matches += 1;
        }
        n += 1;
    }
    println!("{} / {} match", matches, n);
    Ok(())
}

// Structure for one entry that comes in JSON
#[derive(Serialize, Deserialize, Debug)]
struct Entry {
    source: String,
    target: String,
}

// Process one entry, provided in JSON text, by showing original,
// redacted, and target text
fn process_line(text: &str) -> Result<bool> {
    // Parse the JSON into source and target text
    let data: Entry = serde_json::from_str(text)?;

    // Redact the text, check if matches target
    let redacted = redact(&data.source)?;
    let matches = redacted == data.target;

    // Display results if no match
    if !matches {
        println!("Source text:\n{}", data.source);
        println!("\nTarget text:\n{}\n", data.target);
        println!("\nRedacted text:\n{}\n", redacted);
        println!("- - - - -\n");
    }
    Ok(matches)
}
