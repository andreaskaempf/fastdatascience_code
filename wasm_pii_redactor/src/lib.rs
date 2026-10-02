// Rust WebAssembly model for redacting PII substrings in a string by,
// replacing them with tokens.

// Imports for WebAssembly
use wasm_bindgen::prelude::*;

mod redact;
use crate::redact::redact;

// Redact a string by replacing PII with tokens
#[wasm_bindgen]
pub fn wasm_redact(text: &str) -> String {
    redact(text)
}
