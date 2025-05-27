# xdcc-search

A library for querying XDCC file distribution engines, such as [sunxdcc.com](https://sunxdcc.com).

This crate provides a lightweight, asynchronous interface to search XDCC bots and retrieve pack metadata. It parses and normalizes the response into structured Rust types.

## Features

- 🚀 Asynchronous search using `reqwest`
- 📦 Typed `Entry` results with filename, size, bot info, etc.
- 🛠 Error handling for malformed data
- 🔌 Extensible design: future support for multiple XDCC engines

## Example

```rust
use xdcc_search::sunxdcc::Engine;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::default();
    let results = engine.search("ubuntu", 1).await?;

    for entry in results {
        println!(
            "Pack #{} from {} on {}: {} ({} bytes, {} downloads)",
            entry.packnum,
            entry.bot_name,
            entry.network,
            entry.filename,
            entry.filesize,
            entry.downloads
        );
    }

    Ok(())
}
````

## Crate Organization

* `sunxdcc`: Implementation of the search engine for [sunxdcc.com](https://sunxdcc.com).
* `lib.rs`: Re-exports `sunxdcc` and acts as an integration point for future engines.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
xdcc-search = "0.1.0"
```

> Note: Replace `"0.1.0"` with the actual version when publishing to crates.io.

## Roadmap

* ✅ SunXDCC support
* 🔜 Add trait-based abstraction for other engines
* 🔍 Support filtering, sorting, or ranking results
* 🧪 Add unit tests and fuzzing for decoders

## License

MIT OR Apache-2.0

---

> This project is not affiliated with sunxdcc.com or any XDCC indexing service. Use responsibly.
