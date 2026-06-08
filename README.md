# tool-arg-defaults

[![CI](https://github.com/MukundaKatta/tool-arg-defaults-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/MukundaKatta/tool-arg-defaults-rs/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Fill in missing arguments on LLM-generated tool calls with per-tool (and
global) defaults, before the tool function runs.

When a model omits optional arguments, this crate completes the argument object
from a defaults map. The rules are simple and predictable:

- **Caller-supplied values always win.** Anything the model emits is kept.
- **`null` is a real value, not "use the default".** An explicit `null` is
  preserved and never overwritten by a default.
- **Priority is global < tool-specific < caller.** Tool defaults override
  global defaults; caller args override both.
- **Non-object arguments pass through unchanged.** If `args` isn't a JSON
  object, it is returned as-is.

The only dependency is [`serde_json`](https://crates.io/crates/serde_json).

## Install

Add it to your `Cargo.toml`:

```toml
[dependencies]
tool-arg-defaults = "0.1"
serde_json = "1"
```

## Usage

```rust
use serde_json::json;
use tool_arg_defaults::ToolArgDefaults;

let mut defaults = ToolArgDefaults::new();
defaults.set_defaults("search_web", json!({"timeout": 30, "max_results": 10}));
defaults.set_defaults("fetch", json!({"timeout": 60, "follow_redirects": true}));

// The model only passed "q"; the rest come from the tool defaults.
let merged = defaults.apply("search_web", &json!({"q": "anthropic"}));
assert_eq!(merged["q"], "anthropic");
assert_eq!(merged["timeout"], 30);
assert_eq!(merged["max_results"], 10);

// Caller-supplied timeout wins.
let merged = defaults.apply("search_web", &json!({"q": "x", "timeout": 5}));
assert_eq!(merged["timeout"], 5);
assert_eq!(merged["max_results"], 10);

// An explicit null is preserved, not replaced by a default.
let merged = defaults.apply("search_web", &json!({"q": "x", "timeout": null}));
assert!(merged["timeout"].is_null());
```

Global defaults apply to every tool and sit at the lowest priority:

```rust
use serde_json::json;
use tool_arg_defaults::ToolArgDefaults;

let mut d = ToolArgDefaults::new();
d.set_global_defaults(json!({"lang": "en"}));
d.set_defaults("translate", json!({"lang": "fr"})); // overrides global for this tool

assert_eq!(d.apply("search", &json!({}))["lang"], "en");    // global default
assert_eq!(d.apply("translate", &json!({}))["lang"], "fr"); // tool default wins
assert_eq!(d.apply("translate", &json!({"lang": "de"}))["lang"], "de"); // caller wins
```

## API

| Method | Description |
| --- | --- |
| `ToolArgDefaults::new()` / `default()` | Create an empty store. |
| `set_defaults(name, json_object)` | Replace the defaults for a tool. Non-object input is ignored. |
| `merge_defaults(name, json_object)` | Add/override individual default keys for a tool without discarding the rest; creates the entry if missing. |
| `set_global_defaults(json_object)` | Replace the global defaults applied to every tool. |
| `apply(name, args) -> Value` | Merge defaults into `args` (global, then tool, then caller). |
| `apply_global(args) -> Value` | Merge only the global defaults into `args`. |
| `remove_defaults(name) -> bool` | Remove a tool's defaults; returns whether an entry existed. |
| `has_defaults(name) -> bool` | Whether tool-specific defaults are registered. |
| `get_defaults(name) -> Option<&Map>` | Borrow a tool's stored defaults. |
| `global_defaults() -> &Map` | Borrow the global defaults. |
| `tool_names() -> Vec<&str>` | Names of all tools with explicit defaults. |
| `is_empty() -> bool` | Whether no tool or global defaults are set. |
| `clear()` | Remove all tool-specific and global defaults. |

All merging is non-mutating: `apply` and `apply_global` return a new
`serde_json::Value` and never modify the arguments you pass in.

## Development

```sh
cargo test                 # unit + integration + doc tests
cargo fmt --all -- --check # formatting
cargo clippy --all-targets # lints
```

## License

Licensed under either of

- MIT license ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.
