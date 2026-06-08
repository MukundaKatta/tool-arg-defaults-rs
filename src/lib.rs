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

    /// Set (replacing any existing) defaults for a specific tool.
    ///
    /// `defaults` must be a JSON object; any other [`Value`] is ignored and
    /// the call becomes a no-op. Use [`merge_defaults`](Self::merge_defaults)
    /// to add or override individual keys without discarding existing ones.
    pub fn set_defaults(&mut self, tool_name: &str, defaults: Value) {
        if let Value::Object(m) = defaults {
            self.tool_defaults.insert(tool_name.to_owned(), m);
        }
    }

    /// Merge `defaults` into the existing defaults for `tool_name`, creating
    /// the entry if it does not exist. Keys present in `defaults` override the
    /// previously stored values for the same key; other keys are kept.
    ///
    /// Non-object `defaults` are ignored (no-op).
    pub fn merge_defaults(&mut self, tool_name: &str, defaults: Value) {
        if let Value::Object(m) = defaults {
            let entry = self.tool_defaults.entry(tool_name.to_owned()).or_default();
            for (k, v) in m {
                entry.insert(k, v);
            }
        }
    }

    /// Set global defaults applied to every tool call (lowest priority).
    /// Non-object values are ignored (no-op).
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

    /// Remove defaults for a tool. Returns `true` if an entry was removed.
    pub fn remove_defaults(&mut self, tool_name: &str) -> bool {
        self.tool_defaults.remove(tool_name).is_some()
    }

    /// True if defaults are set for this tool.
    pub fn has_defaults(&self, tool_name: &str) -> bool {
        self.tool_defaults.contains_key(tool_name)
    }

    /// Borrow the stored defaults for `tool_name`, if any.
    pub fn get_defaults(&self, tool_name: &str) -> Option<&Map<String, Value>> {
        self.tool_defaults.get(tool_name)
    }

    /// Borrow the global defaults map.
    pub fn global_defaults(&self) -> &Map<String, Value> {
        &self.global_defaults
    }

    /// All tool names that have explicit defaults. Order is unspecified.
    pub fn tool_names(&self) -> Vec<&str> {
        self.tool_defaults.keys().map(|s| s.as_str()).collect()
    }

    /// True if no tool-specific and no global defaults are registered.
    pub fn is_empty(&self) -> bool {
        self.tool_defaults.is_empty() && self.global_defaults.is_empty()
    }

    /// Remove all tool-specific and global defaults.
    pub fn clear(&mut self) {
        self.tool_defaults.clear();
        self.global_defaults.clear();
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
        assert!(d.remove_defaults("t"));
        let out = d.apply("t", &json!({}));
        assert!(out.get("x").is_none());
    }

    #[test]
    fn remove_defaults_returns_false_when_absent() {
        let mut d = ToolArgDefaults::new();
        assert!(!d.remove_defaults("nope"));
    }

    #[test]
    fn merge_defaults_creates_and_extends() {
        let mut d = ToolArgDefaults::new();
        d.set_defaults("t", json!({"a": 1, "b": 2}));
        // Override "b", add "c", keep "a".
        d.merge_defaults("t", json!({"b": 20, "c": 3}));
        let out = d.apply("t", &json!({}));
        assert_eq!(out["a"], 1);
        assert_eq!(out["b"], 20);
        assert_eq!(out["c"], 3);
    }

    #[test]
    fn merge_defaults_on_unknown_tool_registers_it() {
        let mut d = ToolArgDefaults::new();
        d.merge_defaults("fresh", json!({"k": "v"}));
        assert!(d.has_defaults("fresh"));
        assert_eq!(d.apply("fresh", &json!({}))["k"], "v");
    }

    #[test]
    fn non_object_defaults_are_ignored() {
        let mut d = ToolArgDefaults::new();
        d.set_defaults("t", json!([1, 2, 3]));
        assert!(!d.has_defaults("t"));
        d.set_global_defaults(json!("not an object"));
        assert!(d.global_defaults().is_empty());
    }

    #[test]
    fn get_defaults_borrows_stored_map() {
        let mut d = ToolArgDefaults::new();
        d.set_defaults("t", json!({"x": 1}));
        assert_eq!(d.get_defaults("t").unwrap()["x"], 1);
        assert!(d.get_defaults("missing").is_none());
    }

    #[test]
    fn is_empty_reflects_state() {
        let mut d = ToolArgDefaults::new();
        assert!(d.is_empty());
        d.set_defaults("t", json!({"x": 1}));
        assert!(!d.is_empty());
    }

    #[test]
    fn clear_removes_everything() {
        let mut d = ToolArgDefaults::new();
        d.set_defaults("t", json!({"x": 1}));
        d.set_global_defaults(json!({"g": 1}));
        d.clear();
        assert!(d.is_empty());
        assert!(!d.has_defaults("t"));
        assert!(d.global_defaults().is_empty());
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
