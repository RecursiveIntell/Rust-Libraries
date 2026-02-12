use crate::types::PipelineContext;

/// Build a prompt string with variable substitution.
///
/// Replaces `{key}` placeholders in the template with values from the context.
/// The special `{input}` placeholder is replaced by the `input` parameter.
pub fn render(template: &str, input: &str, context: &PipelineContext) -> String {
    let mut rendered = template.replace("{input}", input);
    for (key, value) in &context.data {
        let placeholder = format!("{{{}}}", key);
        rendered = rendered.replace(&placeholder, value);
    }
    rendered
}

/// Create a numbered list from items (1-indexed).
pub fn numbered_list(items: &[String]) -> String {
    items
        .iter()
        .enumerate()
        .map(|(i, item)| format!("{}. {}", i + 1, item))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Wrap text in a labeled section for structured prompts.
pub fn section(label: &str, content: &str) -> String {
    format!("## {}\n{}", label, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_basic() {
        let ctx = PipelineContext::new().insert("name", "Alice");
        let result = render("Hello {name}, process {input}", "data", &ctx);
        assert_eq!(result, "Hello Alice, process data");
    }

    #[test]
    fn test_render_no_placeholders() {
        let ctx = PipelineContext::new();
        let result = render("static prompt", "ignored_in_template", &ctx);
        assert_eq!(result, "static prompt");
    }

    #[test]
    fn test_numbered_list() {
        let items = vec![
            "First".to_string(),
            "Second".to_string(),
            "Third".to_string(),
        ];
        let result = numbered_list(&items);
        assert_eq!(result, "1. First\n2. Second\n3. Third");
    }

    #[test]
    fn test_numbered_list_empty() {
        let result = numbered_list(&[]);
        assert_eq!(result, "");
    }

    #[test]
    fn test_section() {
        let result = section("Context", "Some knowledge here");
        assert_eq!(result, "## Context\nSome knowledge here");
    }
}
