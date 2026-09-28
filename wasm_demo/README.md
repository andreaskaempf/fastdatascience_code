# Building WebAssembly Browser Plugins in Rust

This is a relatively simple setup that allows you to create WebAssembly modules that
can be loaded and used in the browser. It is simpler than the usual pipeline, because
it does not require Node.js or any other non-Rust dependencies.

Setup
- `cargo new --lib hello-wasm`
- Creates new project with code in src/lib.rs, which we will replace
- `cd hello-wasm`
- `cargo install wasm-bindgen-cli` (installs globally)
- `cargo add wasm-bindgen`

Edit Rust program src/lib.rs
- Note that `#[wasm_bindgen]` makes Rust function callable from JavaScript

```
use wasm_bindgen::prelude::*;

// Format a string to say hello
#[wasm_bindgen]
pub fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}

// Convert string by replacing instances of "aaa" with "XXX"
#[wasm_bindgen]
pub fn convert(text: &str) -> String {
    text.replace("aaa", "XXX")
}
```

Build project
- `cargo build --release --target wasm32-unknown-unknown`
- `wasm-bindgen --target web --out-dir pkg target/wasm32-unknown-unknown/release/hello_wasm.wasm`
- Creates output in ./pkg
- Check that Cargo.toml contains [lib] \n crate-type = ["cdylib"]

Show as web page
- See index.html for how to load and run the module
- Run behind a web server, will not work as file://

Sample index.html:

```
<!doctype html>
<html lang="en-US">
  <head>
    <meta charset="utf-8" />
    <title>WASM example</title>
  </head>
  <body>

    <p>Click me button to say hello:
        <button id="button1">Button</button>
    </p>

    <p>Enter text, aaa will be converted:</p>
    <textarea id="text1" style="width: 100%; height: 200px"></textarea>

    <script type="module">

      // Import from the WASM module and its JavaScript glue
      import init, { greet, convert } from "./pkg/hello_wasm.js";
      await init();

      function change_text() {
        var t = document.getElementById("text1");
        t.value = convert(t.value);
      }

      function hello() {
        alert(greet("Hello from Wasm"));
      }

      // Attach event handlers to elements (cannot use in-line handlers here)
      document.getElementById("button1").addEventListener("click", hello);
      document.getElementById("text1").addEventListener("input", change_text);

    </script>

  </body>
</html>
```


Sources: 
1. https://developer.mozilla.org/en-US/docs/WebAssembly/Guides/Rust_to_Wasm
2. https://dev.to/dandyvica/wasm-in-rust-without-nodejs-2e0c

