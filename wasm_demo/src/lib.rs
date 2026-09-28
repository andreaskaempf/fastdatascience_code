// Source: 
// https://developer.mozilla.org/en-US/docs/WebAssembly/Guides/Rust_to_Wasm
//
// wasm-pack uses wasm-bindgen to bridge between JavaScript and Rust types. 
// It allows JavaScript to call a Rust API with a string, or a Rust function 
// to catch a JavaScript exception.
use wasm_bindgen::prelude::*;

// For creating a Rust function that JavaScript can call
#[wasm_bindgen]
pub fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}

// Convert string by replacing aaa to XXX
#[wasm_bindgen]
pub fn convert(text: &str) -> String {
    text.replace("aaa", "XXX")
}
