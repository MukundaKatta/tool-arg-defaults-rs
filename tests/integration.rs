//! Integration tests exercising the public API exactly as a downstream
//! consumer of the crate would, through `tool_arg_defaults::ToolArgDefaults`.

use serde_json::json;
use tool_arg_defaults::ToolArgDefaults;

#[test]
fn end_to_end_tool_call_flow() {
    // Simulate an agent runtime registering defaults for two tools, then an
    // LLM emitting partial argument objects that get completed before dispatch.
    let mut d = ToolArgDefaults::new();
    d.set_defaults("search_web", json!({"timeout": 30, "max_results": 10}));
    d.set_defaults("fetch", json!({"timeout": 60, "follow_redirects": true}));

    // LLM only supplied the query.
    let merged = d.apply("search_web", &json!({"q": "anthropic"}));
    assert_eq!(merged["q"], "anthropic");
    assert_eq!(merged["timeout"], 30);
    assert_eq!(merged["max_results"], 10);

    // Caller-supplied timeout overrides the default.
    let merged = d.apply("search_web", &json!({"q": "x", "timeout": 5}));
    assert_eq!(merged["timeout"], 5);
    assert_eq!(merged["max_results"], 10);

    // A different tool uses its own defaults, not the other tool's.
    let merged = d.apply("fetch", &json!({"url": "https://example.com"}));
    assert_eq!(merged["timeout"], 60);
    assert_eq!(merged["follow_redirects"], true);
    assert!(merged.get("max_results").is_none());
}

#[test]
fn null_is_treated_as_an_explicit_value() {
    let mut d = ToolArgDefaults::new();
    d.set_defaults("t", json!({"region": "us-east-1"}));

    // Explicit null must NOT be replaced by the default.
    let merged = d.apply("t", &json!({"region": null}));
    assert!(merged["region"].is_null());
}

#[test]
fn priority_order_global_then_tool_then_caller() {
    let mut d = ToolArgDefaults::new();
    d.set_global_defaults(json!({"lang": "en", "trace": false, "shared": "global"}));
    d.set_defaults("t", json!({"trace": true, "shared": "tool"}));

    let merged = d.apply("t", &json!({"shared": "caller"}));
    // Global-only key survives.
    assert_eq!(merged["lang"], "en");
    // Tool default overrides global default.
    assert_eq!(merged["trace"], true);
    // Caller overrides both.
    assert_eq!(merged["shared"], "caller");
}

#[test]
fn merge_then_apply_keeps_and_overrides_keys() {
    let mut d = ToolArgDefaults::new();
    d.set_defaults("db", json!({"retries": 3, "timeout": 10}));
    d.merge_defaults("db", json!({"timeout": 25, "pool": 8}));

    let merged = d.apply("db", &json!({}));
    assert_eq!(merged["retries"], 3); // untouched
    assert_eq!(merged["timeout"], 25); // overridden by merge
    assert_eq!(merged["pool"], 8); // added by merge
}

#[test]
fn non_object_arguments_pass_through_unchanged() {
    let mut d = ToolArgDefaults::new();
    d.set_defaults("t", json!({"x": 1}));

    for value in [json!(42), json!("string"), json!([1, 2, 3]), json!(null)] {
        assert_eq!(d.apply("t", &value), value);
    }
}

#[test]
fn apply_does_not_mutate_caller_args() {
    let mut d = ToolArgDefaults::new();
    d.set_defaults("t", json!({"added": true}));

    let original = json!({"q": "hi"});
    let _ = d.apply("t", &original);
    // The original argument object is unchanged after apply.
    assert_eq!(original, json!({"q": "hi"}));
}

#[test]
fn default_trait_matches_new() {
    let from_new = ToolArgDefaults::new();
    let from_default = ToolArgDefaults::default();
    assert!(from_new.is_empty());
    assert!(from_default.is_empty());
}
