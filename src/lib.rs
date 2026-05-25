/*!
tool-arg-defaults: fill in missing kwargs on LLM-generated tool calls.

LLMs often omit optional args. `ToolDefaults` merges per-tool default
values into the LLM's args before execution. Caller-supplied keys always
win — including explicit `null`.

```rust
use serde_json::json;
use tool_arg_defaults::ToolDefaults;

let mut defaults = ToolDefaults::new();
defaults.register("search_web", json!({"timeout": 30, "max_results": 10}));

// LLM only passed "q"
let merged = defaults.apply("search_web", &json!({"q": "anthropic"}), false).unwrap();
assert_eq!(merged["timeout"], json!(30));
assert_eq!(merged["max_results"], json!(10));
assert_eq!(merged["q"], json!("anthropic"));

// Caller-supplied value wins
let merged = defaults.apply("search_web", &json!({"q": "x", "timeout": 5}), false).unwrap();
assert_eq!(merged["timeout"], json!(5));
```
*/

use serde_json::{Map, Value};
use std::collections::HashMap;

// ---- error ----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolNotRegisteredError(pub String);

impl std::fmt::Display for ToolNotRegisteredError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tool {:?} is not registered", self.0)
    }
}

impl std::error::Error for ToolNotRegisteredError {}

// ---- ToolDefaults ---------------------------------------------------------

/// Per-tool default kwargs for LLM-generated tool calls.
///
/// Defaults are stored as JSON objects (`serde_json::Map<String, Value>`).
/// `apply` merges them into the caller-supplied args; caller-supplied keys win.
/// `null` is a real value — passing `{"timeout": null}` will NOT be replaced
/// by the registered default. To get the default, omit the key entirely.
#[derive(Debug, Default, Clone)]
pub struct ToolDefaults {
    defaults: HashMap<String, Map<String, Value>>,
}

impl ToolDefaults {
    pub fn new() -> Self {
        Self::default()
    }

    // ---- registration -----------------------------------------------

    /// Register (or overwrite) defaults for `tool_name`.
    ///
    /// `defaults` must be a JSON object (`Value::Object`); panics otherwise.
    pub fn register(&mut self, tool_name: impl Into<String>, defaults: Value) {
        let obj = match defaults {
            Value::Object(m) => m,
            other => panic!("defaults must be a JSON object, got {other:?}"),
        };
        self.defaults.insert(tool_name.into(), obj);
    }

    /// Register multiple tools at once from a JSON object of objects.
    pub fn register_many(&mut self, map: Value) {
        if let Value::Object(outer) = map {
            for (name, val) in outer {
                self.register(name, val);
            }
        }
    }

    /// Merge new defaults into an existing registration (or create one).
    pub fn update(&mut self, tool_name: impl Into<String>, extra: Value) {
        let name = tool_name.into();
        let target = self.defaults.entry(name).or_default();
        if let Value::Object(m) = extra {
            target.extend(m);
        }
    }

    /// Drop a tool's defaults. Returns true if it was registered.
    pub fn unregister(&mut self, tool_name: &str) -> bool {
        self.defaults.remove(tool_name).is_some()
    }

    // ---- inspection -------------------------------------------------

    /// Sorted list of registered tool names.
    pub fn tool_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.defaults.keys().map(|s| s.as_str()).collect();
        names.sort_unstable();
        names
    }

    /// Copy of the defaults for `tool_name`.
    pub fn defaults_for(&self, tool_name: &str) -> Option<Map<String, Value>> {
        self.defaults.get(tool_name).cloned()
    }

    pub fn contains(&self, tool_name: &str) -> bool {
        self.defaults.contains_key(tool_name)
    }

    // ---- core -------------------------------------------------------

    /// Merge defaults into `args`. Caller-supplied keys win.
    ///
    /// `args` must be a JSON object or `Value::Null` (treated as empty).
    /// Returns `Err(ToolNotRegisteredError)` if `strict=true` and tool not found.
    /// If `strict=false` and tool not found, returns `args` unchanged.
    pub fn apply(
        &self,
        tool_name: &str,
        args: &Value,
        strict: bool,
    ) -> Result<Value, ToolNotRegisteredError> {
        let supplied: Map<String, Value> = match args {
            Value::Object(m) => m.clone(),
            Value::Null => Map::new(),
            _ => Map::new(),
        };
        let Some(tool_defaults) = self.defaults.get(tool_name) else {
            if strict {
                return Err(ToolNotRegisteredError(tool_name.to_owned()));
            }
            return Ok(Value::Object(supplied));
        };
        // Start from defaults, then overwrite with caller-supplied values.
        let mut merged = tool_defaults.clone();
        for (k, v) in supplied {
            merged.insert(k, v);
        }
        Ok(Value::Object(merged))
    }
}

// ---- tests ----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fill_missing_args() {
        let mut d = ToolDefaults::new();
        d.register("search", json!({"timeout": 30, "max_results": 10}));
        let out = d.apply("search", &json!({"q": "hello"}), false).unwrap();
        assert_eq!(out["q"], json!("hello"));
        assert_eq!(out["timeout"], json!(30));
        assert_eq!(out["max_results"], json!(10));
    }

    #[test]
    fn caller_wins_on_conflict() {
        let mut d = ToolDefaults::new();
        d.register("search", json!({"timeout": 30}));
        let out = d
            .apply("search", &json!({"timeout": 5, "q": "x"}), false)
            .unwrap();
        assert_eq!(out["timeout"], json!(5));
    }

    #[test]
    fn null_is_real_value() {
        let mut d = ToolDefaults::new();
        d.register("fetch", json!({"follow_redirects": true}));
        let out = d
            .apply("fetch", &json!({"follow_redirects": null}), false)
            .unwrap();
        assert_eq!(out["follow_redirects"], Value::Null);
    }

    #[test]
    fn unknown_tool_non_strict() {
        let d = ToolDefaults::new();
        let out = d.apply("ghost", &json!({"x": 1}), false).unwrap();
        assert_eq!(out["x"], json!(1));
    }

    #[test]
    fn unknown_tool_strict() {
        let d = ToolDefaults::new();
        let err = d.apply("ghost", &json!({}), true).unwrap_err();
        assert_eq!(err.0, "ghost");
    }

    #[test]
    fn empty_args_fills_all_defaults() {
        let mut d = ToolDefaults::new();
        d.register("tool", json!({"a": 1, "b": 2}));
        let out = d.apply("tool", &json!({}), false).unwrap();
        assert_eq!(out["a"], json!(1));
        assert_eq!(out["b"], json!(2));
    }

    #[test]
    fn null_args_treated_as_empty() {
        let mut d = ToolDefaults::new();
        d.register("tool", json!({"x": 99}));
        let out = d.apply("tool", &Value::Null, false).unwrap();
        assert_eq!(out["x"], json!(99));
    }

    #[test]
    fn register_overwrites() {
        let mut d = ToolDefaults::new();
        d.register("t", json!({"a": 1}));
        d.register("t", json!({"a": 99, "b": 2}));
        let out = d.apply("t", &json!({}), false).unwrap();
        assert_eq!(out["a"], json!(99));
        assert_eq!(out["b"], json!(2));
    }

    #[test]
    fn update_merges() {
        let mut d = ToolDefaults::new();
        d.register("t", json!({"a": 1}));
        d.update("t", json!({"b": 2}));
        let out = d.apply("t", &json!({}), false).unwrap();
        assert_eq!(out["a"], json!(1));
        assert_eq!(out["b"], json!(2));
    }

    #[test]
    fn update_creates_if_absent() {
        let mut d = ToolDefaults::new();
        d.update("new_tool", json!({"x": 42}));
        let out = d.apply("new_tool", &json!({}), false).unwrap();
        assert_eq!(out["x"], json!(42));
    }

    #[test]
    fn unregister_returns_true_if_was_registered() {
        let mut d = ToolDefaults::new();
        d.register("t", json!({"a": 1}));
        assert!(d.unregister("t"));
        assert!(!d.unregister("t")); // already gone
    }

    #[test]
    fn unregister_then_non_strict() {
        let mut d = ToolDefaults::new();
        d.register("t", json!({"a": 1}));
        d.unregister("t");
        let out = d.apply("t", &json!({"x": 7}), false).unwrap();
        assert_eq!(out["x"], json!(7));
        assert!(!out.as_object().unwrap().contains_key("a"));
    }

    #[test]
    fn tool_names_sorted() {
        let mut d = ToolDefaults::new();
        d.register("zz", json!({}));
        d.register("aa", json!({}));
        d.register("mm", json!({}));
        assert_eq!(d.tool_names(), vec!["aa", "mm", "zz"]);
    }

    #[test]
    fn defaults_for_some() {
        let mut d = ToolDefaults::new();
        d.register("t", json!({"x": 1}));
        let m = d.defaults_for("t").unwrap();
        assert_eq!(m["x"], json!(1));
    }

    #[test]
    fn defaults_for_none() {
        let d = ToolDefaults::new();
        assert!(d.defaults_for("missing").is_none());
    }

    #[test]
    fn contains_check() {
        let mut d = ToolDefaults::new();
        assert!(!d.contains("t"));
        d.register("t", json!({}));
        assert!(d.contains("t"));
        d.unregister("t");
        assert!(!d.contains("t"));
    }

    #[test]
    fn register_many() {
        let mut d = ToolDefaults::new();
        d.register_many(json!({
            "a": {"x": 1},
            "b": {"y": 2}
        }));
        assert_eq!(d.apply("a", &json!({}), false).unwrap()["x"], json!(1));
        assert_eq!(d.apply("b", &json!({}), false).unwrap()["y"], json!(2));
    }

    #[test]
    fn apply_does_not_mutate_args() {
        let mut d = ToolDefaults::new();
        d.register("t", json!({"a": 1}));
        let args = json!({"b": 2});
        let _ = d.apply("t", &args, false).unwrap();
        assert!(!args.as_object().unwrap().contains_key("a"));
    }

    #[test]
    fn nested_value_preserved() {
        let mut d = ToolDefaults::new();
        d.register("t", json!({"opts": {"retries": 3}}));
        let out = d.apply("t", &json!({}), false).unwrap();
        assert_eq!(out["opts"]["retries"], json!(3));
    }

    #[test]
    fn bool_int_string_value_types() {
        let mut d = ToolDefaults::new();
        d.register(
            "t",
            json!({"flag": true, "count": 42, "label": "hi"}),
        );
        let out = d.apply("t", &json!({}), false).unwrap();
        assert_eq!(out["flag"], json!(true));
        assert_eq!(out["count"], json!(42));
        assert_eq!(out["label"], json!("hi"));
    }

    #[test]
    fn multiple_tools_independent() {
        let mut d = ToolDefaults::new();
        d.register("a", json!({"x": 1}));
        d.register("b", json!({"y": 2}));
        let a = d.apply("a", &json!({}), false).unwrap();
        let b = d.apply("b", &json!({}), false).unwrap();
        assert!(!a.as_object().unwrap().contains_key("y"));
        assert!(!b.as_object().unwrap().contains_key("x"));
    }
}
