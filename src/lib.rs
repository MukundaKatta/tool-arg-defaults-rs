/*!
tool-arg-defaults: fill in missing arguments on LLM tool calls.

When an LLM omits optional arguments, this crate fills them in from a
per-tool (or global) defaults map before the tool function is called.
Caller-supplied values always win; `null` is treated as an explicit value
and is not overridden.

```rust
use tool_arg_defaults::ToolArgDefaults;
use serde_json::json;

let mut d = ToolArgDefaults::new();
d.set_defaults("search", json!({"max_results": 10, "format": "json"}));

let args = d.apply("search", &json!({"q": "hello"}));
assert_eq!(args["max_results"], 10);
assert_eq!(args["q"], "hello"); // caller value preserved
```
*/

use serde_json::{Map, Value};
use std::collections::HashMap;

fn merge(base: &Map<String, Value>, overrides: &Map<String, Value>) -> Map<String, Value> {
    let mut result = base.clone();
    for (k, v) in overrides {
        result.insert(k.clone(), v.clone());
    }
    result
}

/// Per-tool default argument store.
pub struct ToolArgDefaults {
    /// Defaults per tool name.
    tool_defaults: HashMap<String, Map<String, Value>>,
    /// Global defaults applied to all tools (tool-specific defaults override these).
    global_defaults: Map<String, Value>,
}

impl Default for ToolArgDefaults {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolArgDefaults {
    pub fn new() -> Self {
        Self {
            tool_defaults: HashMap::new(),
            global_defaults: Map::new(),
        }
    }

    /// Set defaults for a specific tool. `defaults` must be a JSON object.
    /// Panics (debug) if `defaults` is not an object; silently no-ops in
    /// release when the input is not an object.
    pub fn set_defaults(&mut self, tool_name: &str, defaults: Value) {
        if let Value::Object(m) = defaults {
            self.tool_defaults.insert(tool_name.to_owned(), m);
        }
    }

    /// Set global defaults applied to every tool call (lowest priority).
    pub fn set_global_defaults(&mut self, defaults: Value) {
        if let Value::Object(m) = defaults {
            self.global_defaults = m;
        }
    }

    /// Apply defaults for `tool_name` to `args`.
    ///
    /// Priority (highest first):
    /// 1. Caller-supplied args
    /// 2. Tool-specific defaults
    /// 3. Global defaults
    pub fn apply(&self, tool_name: &str, args: &Value) -> Value {
        let caller = match args {
            Value::Object(m) => m.clone(),
            _ => return args.clone(),
        };

        let tool_def = self
            .tool_defaults
            .get(tool_name)
            .cloned()
            .unwrap_or_default();

        // Start from global defaults, apply tool defaults on top, then caller args.
        let base = merge(&self.global_defaults, &tool_def);
        Value::Object(merge(&base, &caller))
    }

    /// Apply only global defaults (tool-name-agnostic).
    pub fn apply_global(&self, args: &Value) -> Value {
        let caller = match args {
            Value::Object(m) => m.clone(),
            _ => return args.clone(),
        };
        Value::Object(merge(&self.global_defaults, &caller))
    }

    /// Remove defaults for a tool.
    pub fn remove_defaults(&mut self, tool_name: &str) {
        self.tool_defaults.remove(tool_name);
    }

    /// True if defaults are set for this tool.
    pub fn has_defaults(&self, tool_name: &str) -> bool {
        self.tool_defaults.contains_key(tool_name)
    }

    /// All tool names that have explicit defaults.
    pub fn tool_names(&self) -> Vec<&str> {
        self.tool_defaults.keys().map(|s| s.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fills_missing_key() {
        let mut d = ToolArgDefaults::new();
        d.set_defaults("t", json!({"x": 1}));
        let out = d.apply("t", &json!({}));
        assert_eq!(out["x"], 1);
    }

    #[test]
    fn caller_wins_over_default() {
        let mut d = ToolArgDefaults::new();
        d.set_defaults("t", json!({"x": 1}));
        let out = d.apply("t", &json!({"x": 99}));
        assert_eq!(out["x"], 99);
    }

    #[test]
    fn null_is_explicit_not_overridden() {
        let mut d = ToolArgDefaults::new();
        d.set_defaults("t", json!({"x": 1}));
        let out = d.apply("t", &json!({"x": null}));
        assert_eq!(out["x"], json!(null));
    }

    #[test]
    fn no_defaults_returns_args_unchanged() {
        let d = ToolArgDefaults::new();
        let args = json!({"q": "hello"});
        assert_eq!(d.apply("unknown", &args), args);
    }

    #[test]
    fn global_defaults_applied() {
        let mut d = ToolArgDefaults::new();
        d.set_global_defaults(json!({"lang": "en"}));
        let out = d.apply("any_tool", &json!({"q": "hi"}));
        assert_eq!(out["lang"], "en");
        assert_eq!(out["q"], "hi");
    }

    #[test]
    fn tool_defaults_override_global() {
        let mut d = ToolArgDefaults::new();
        d.set_global_defaults(json!({"x": 1}));
        d.set_defaults("t", json!({"x": 2}));
        let out = d.apply("t", &json!({}));
        assert_eq!(out["x"], 2);
    }

    #[test]
    fn caller_wins_over_global() {
        let mut d = ToolArgDefaults::new();
        d.set_global_defaults(json!({"x": 1}));
        let out = d.apply("t", &json!({"x": 42}));
        assert_eq!(out["x"], 42);
    }

    #[test]
    fn apply_global_uses_only_global() {
        let mut d = ToolArgDefaults::new();
        d.set_global_defaults(json!({"lang": "en"}));
        d.set_defaults("t", json!({"tool_only": true}));
        let out = d.apply_global(&json!({}));
        assert_eq!(out["lang"], "en");
        assert!(out.get("tool_only").is_none());
    }

    #[test]
    fn non_object_args_returned_unchanged() {
        let d = ToolArgDefaults::new();
        let args = json!([1, 2, 3]);
        assert_eq!(d.apply("t", &args), args);
    }

    #[test]
    fn remove_defaults_works() {
        let mut d = ToolArgDefaults::new();
        d.set_defaults("t", json!({"x": 1}));
        d.remove_defaults("t");
        let out = d.apply("t", &json!({}));
        assert!(out.get("x").is_none());
    }

    #[test]
    fn has_defaults_returns_true() {
        let mut d = ToolArgDefaults::new();
        d.set_defaults("t", json!({"x": 1}));
        assert!(d.has_defaults("t"));
    }

    #[test]
    fn has_defaults_returns_false() {
        let d = ToolArgDefaults::new();
        assert!(!d.has_defaults("t"));
    }

    #[test]
    fn tool_names_lists_known_tools() {
        let mut d = ToolArgDefaults::new();
        d.set_defaults("a", json!({}));
        d.set_defaults("b", json!({}));
        let mut names = d.tool_names();
        names.sort();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn extra_caller_keys_preserved() {
        let mut d = ToolArgDefaults::new();
        d.set_defaults("t", json!({"x": 1}));
        let out = d.apply("t", &json!({"y": 2}));
        assert_eq!(out["x"], 1);
        assert_eq!(out["y"], 2);
    }

    #[test]
    fn empty_defaults_not_added_to_output() {
        let d = ToolArgDefaults::new();
        let out = d.apply("t", &json!({"a": 1}));
        assert_eq!(out.as_object().unwrap().len(), 1);
    }
}
