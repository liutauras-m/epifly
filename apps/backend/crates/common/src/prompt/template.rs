//! Simple mustache-like prompt template: `{{key.subkey}}` interpolation.

use serde_json::Value;

#[derive(Debug, Clone)]
pub struct PromptTemplate {
    template: String,
}

impl PromptTemplate {
    pub fn new(template: impl Into<String>) -> Self {
        Self { template: template.into() }
    }

    /// Render the template, replacing `{{path.to.key}}` with values from `ctx`.
    ///
    /// Walks dot-separated paths in the JSON `ctx`. Missing paths are replaced with
    /// an empty string. Values are rendered as compact JSON scalars (strings without
    /// surrounding quotes, numbers/booleans as-is).
    pub fn render(&self, ctx: &Value) -> String {
        let mut start = 0;
        let mut output = String::with_capacity(self.template.len() * 2);
        let tmpl = self.template.as_str();
        while let Some(open) = tmpl[start..].find("{{") {
            let abs_open = start + open;
            if let Some(close) = tmpl[abs_open..].find("}}") {
                let abs_close = abs_open + close;
                output.push_str(&tmpl[start..abs_open]);
                let path = &tmpl[abs_open + 2..abs_close];
                output.push_str(&resolve_path(ctx, path.trim()));
                start = abs_close + 2;
            } else {
                break;
            }
        }
        output.push_str(&tmpl[start..]);
        output
    }
}

fn resolve_path(ctx: &Value, path: &str) -> String {
    let mut cur = ctx;
    for key in path.split('.') {
        match cur {
            Value::Object(map) => {
                if let Some(v) = map.get(key) {
                    cur = v;
                } else {
                    return String::new();
                }
            }
            _ => return String::new(),
        }
    }
    match cur {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn simple_substitution() {
        let t = PromptTemplate::new("Hello {{name}}!");
        let ctx = json!({ "name": "World" });
        assert_eq!(t.render(&ctx), "Hello World!");
    }

    #[test]
    fn nested_path() {
        let t = PromptTemplate::new("ID: {{tenant.id}}");
        let ctx = json!({ "tenant": { "id": "abc" } });
        assert_eq!(t.render(&ctx), "ID: abc");
    }

    #[test]
    fn missing_key_is_empty() {
        let t = PromptTemplate::new("{{missing}} end");
        let ctx = json!({});
        assert_eq!(t.render(&ctx), " end");
    }

    #[test]
    fn number_value() {
        let t = PromptTemplate::new("count={{input.count}}");
        let ctx = json!({ "input": { "count": 42 } });
        assert_eq!(t.render(&ctx), "count=42");
    }

    #[test]
    fn multiple_placeholders() {
        let t = PromptTemplate::new("{{a}} + {{b}} = {{c}}");
        let ctx = json!({ "a": "1", "b": "2", "c": "3" });
        assert_eq!(t.render(&ctx), "1 + 2 = 3");
    }
}
