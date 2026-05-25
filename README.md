# tool-arg-defaults

Apply per-tool default kwargs to LLM-generated tool calls. Caller-supplied values always win. `null` is a real value, not "use the default".

## Usage

```rust
use serde_json::json;
use tool_arg_defaults::ToolDefaults;

let mut defaults = ToolDefaults::new();
defaults.register("search_web", json!({"timeout": 30, "max_results": 10}));
defaults.register("fetch", json!({"timeout": 60, "follow_redirects": true}));

// LLM only passed "q"
let merged = defaults.apply("search_web", &json!({"q": "anthropic"}), false).unwrap();
// merged == {"q": "anthropic", "timeout": 30, "max_results": 10}

// Caller-supplied timeout wins
let merged = defaults.apply("search_web", &json!({"q": "x", "timeout": 5}), false).unwrap();
// merged == {"q": "x", "timeout": 5, "max_results": 10}
```

## Features

- `register(name, json_object)` sets defaults for a tool
- `apply(name, args, strict)` merges defaults into caller-supplied args
- Caller keys always win (including explicit `null`)
- `strict=true` raises `ToolNotRegisteredError` for unknown tools
- `update` / `unregister` for dynamic reconfiguration
- Zero dependencies beyond `serde_json`

## License

MIT OR Apache-2.0
