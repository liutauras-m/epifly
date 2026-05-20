/// Minimal variable substitution for storage path templates.
///
/// Supported placeholders:
/// - `{tenant_id}` → tenant ID string
/// - `{user_id}` → user ID string (or `__dev__` if absent)
/// - `{now:%Y/%m/%d}` → current UTC date formatted with `chrono` strftime syntax
///
/// Unknown placeholders are passed through unchanged.
pub fn render(template: &str, tenant_id: &str, user_id: Option<&str>) -> String {
    let mut out = template.to_owned();

    out = out.replace("{tenant_id}", tenant_id);
    out = out.replace("{user_id}", user_id.unwrap_or("__dev__"));

    // Handle {now:<fmt>} — replace with formatted UTC timestamp.
    while let Some(start) = out.find("{now:") {
        let rest = &out[start + 5..];
        let Some(end_offset) = rest.find('}') else {
            break;
        };
        let fmt = &rest[..end_offset];
        let formatted = chrono::Utc::now().format(fmt).to_string();
        let placeholder = format!("{{now:{fmt}}}");
        out = out.replacen(&placeholder, &formatted, 1);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_substitution() {
        let out = render("uploads/{tenant_id}/{user_id}", "acme", Some("alice"));
        assert_eq!(out, "uploads/acme/alice");
    }

    #[test]
    fn now_format() {
        let out = render("data/{now:%Y}", "t1", None);
        let year = chrono::Utc::now().format("%Y").to_string();
        assert_eq!(out, format!("data/{year}"));
    }

    #[test]
    fn unknown_placeholder_preserved() {
        let out = render("files/{unknown}/doc.md", "t1", None);
        assert_eq!(out, "files/{unknown}/doc.md");
    }
}
