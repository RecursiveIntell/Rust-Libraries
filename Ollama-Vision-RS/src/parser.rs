//! Robust LLM response parser with 7-strategy tag extraction.
//!
//! Handles every common LLM output format:
//! 1. Pure JSON array: `["tag1", "tag2"]`
//! 2. JSON after `<think>` blocks: `<think>...</think>["tag1", "tag2"]`
//! 3. Markdown code blocks: `` ```json\n["tag1"]\n``` ``
//! 4. JSON object with "tags" key: `{"tags": ["tag1", "tag2"]}`
//! 5. Bracket-matched array extraction from surrounding text
//! 6. Numbered/bulleted list extraction
//! 7. Comma-separated fallback: `tag1, tag2, tag3`

/// Parse an LLM response into a list of tags using 7 strategies.
///
/// Strategies are tried in order from most structured to least:
/// 1. Direct JSON array parse
/// 2. Strip `<think>` blocks, then JSON array
/// 3. JSON object with "tags" key
/// 4. Markdown code block extraction
/// 5. Bracket-matched JSON array search
/// 6. Line-based list extraction (numbered/bulleted)
/// 7. Comma-separated fallback
pub fn parse_tags(response: &str) -> Result<Vec<String>, ParseError> {
    let trimmed = response.trim();

    if trimmed.is_empty() {
        return Err(ParseError::EmptyResponse);
    }

    // Strategy 1: Direct JSON array
    if let Ok(arr) = serde_json::from_str::<Vec<String>>(trimmed) {
        return Ok(clean_tags(arr));
    }

    // Strategy 2: Strip <think>...</think> blocks
    let cleaned = strip_think_tags(trimmed);
    let cleaned = cleaned.trim();

    if let Ok(arr) = serde_json::from_str::<Vec<String>>(cleaned) {
        return Ok(clean_tags(arr));
    }

    // Strategy 3: JSON object with "tags" key
    if let Some(tags) = try_extract_tags_from_object(cleaned) {
        return Ok(clean_tags(tags));
    }

    // Strategy 4: Markdown code block extraction
    if let Some(tags) = extract_tags_from_code_block(cleaned) {
        return Ok(clean_tags(tags));
    }

    // Strategy 5: Bracket-matched JSON array search
    if let Some(tags) = find_json_array(cleaned) {
        return Ok(clean_tags(tags));
    }

    // Strategy 6: Line-based list extraction (numbered/bulleted)
    if let Some(tags) = extract_from_list(cleaned) {
        return Ok(clean_tags(tags));
    }

    // Strategy 7: Comma-separated fallback
    let tags: Vec<String> = cleaned
        .split(',')
        .map(|s| s.trim().trim_matches('"').trim().to_lowercase())
        .filter(|s| !s.is_empty() && s.len() < 50)
        .collect();

    if tags.is_empty() {
        return Err(ParseError::Unparseable(cleaned.to_string()));
    }

    Ok(tags)
}

/// Strip `<think>...</think>` blocks emitted by reasoning models.
///
/// Handles both complete and incomplete think blocks:
/// - `<think>reasoning</think>content` -> `content`
/// - `<think>reasoning without closing` -> `` (strips to end)
pub fn strip_think_tags(text: &str) -> String {
    let mut result = text.to_string();
    while let Some(start) = result.find("<think>") {
        if let Some(end) = result[start..].find("</think>") {
            result = format!("{}{}", &result[..start], &result[start + end + 8..]);
        } else {
            // No closing tag — strip from <think> to end
            result = result[..start].to_string();
            break;
        }
    }
    result
}

/// Parse error types for tag extraction.
#[derive(Debug)]
pub enum ParseError {
    /// The response was empty or whitespace-only
    EmptyResponse,
    /// None of the 7 strategies could extract tags
    Unparseable(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::EmptyResponse => write!(f, "Empty LLM response"),
            ParseError::Unparseable(s) => {
                write!(f, "Could not parse tags from LLM response: {}", s)
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// Try parsing as a JSON object and extracting an array from a "tags" key.
fn try_extract_tags_from_object(text: &str) -> Option<Vec<String>> {
    let val: serde_json::Value = serde_json::from_str(text).ok()?;
    let arr = val.get("tags").and_then(|v| v.as_array())?;
    let tags = arr
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();
    Some(tags)
}

/// Extract a JSON array from markdown code blocks.
fn extract_tags_from_code_block(text: &str) -> Option<Vec<String>> {
    for marker in ["```json", "```"] {
        let mut search_from = 0;
        while let Some(start) = text[search_from..].find(marker) {
            let abs_start = search_from + start + marker.len();
            let content_start = text[abs_start..].find('\n').map(|p| abs_start + p + 1)?;
            if let Some(end) = text[content_start..].find("```") {
                let candidate = text[content_start..content_start + end].trim();
                if let Ok(arr) = serde_json::from_str::<Vec<String>>(candidate) {
                    return Some(arr);
                }
                if let Some(tags) = try_extract_tags_from_object(candidate) {
                    return Some(tags);
                }
            }
            search_from = abs_start;
        }
    }
    None
}

/// Find a JSON array by bracket matching, preferring later occurrences.
fn find_json_array(text: &str) -> Option<Vec<String>> {
    let starts: Vec<usize> = text.match_indices('[').map(|(i, _)| i).collect();
    let ends: Vec<usize> = text.match_indices(']').map(|(i, _)| i).collect();

    for &start in starts.iter().rev() {
        for &end in ends.iter().rev() {
            if end <= start {
                continue;
            }
            let candidate = &text[start..=end];
            if let Ok(arr) = serde_json::from_str::<Vec<String>>(candidate) {
                return Some(arr);
            }
        }
    }
    None
}

/// Extract tags from numbered or bulleted lists.
///
/// Handles formats like:
/// - `1. tag one`
/// - `- tag two`
/// - `* tag three`
/// - `• tag four`
fn extract_from_list(text: &str) -> Option<Vec<String>> {
    let lines: Vec<&str> = text.lines().collect();
    let list_items: Vec<String> = lines
        .iter()
        .filter_map(|line| {
            let trimmed = line.trim();
            // Numbered: "1. tag", "2) tag"
            if let Some(rest) = trimmed
                .strip_prefix(|c: char| c.is_ascii_digit())
                .and_then(|s| {
                    // Handle multi-digit numbers
                    let s = s.trim_start_matches(|c: char| c.is_ascii_digit());
                    s.strip_prefix('.')
                        .or_else(|| s.strip_prefix(')'))
                })
            {
                let tag = rest.trim().trim_matches('"').trim();
                if !tag.is_empty() {
                    return Some(tag.to_string());
                }
            }
            // Bulleted: "- tag", "* tag", "• tag"
            for prefix in ["-", "*", "•"] {
                if let Some(rest) = trimmed.strip_prefix(prefix) {
                    let tag = rest.trim().trim_matches('"').trim();
                    if !tag.is_empty() {
                        return Some(tag.to_string());
                    }
                }
            }
            None
        })
        .collect();

    if list_items.len() >= 2 {
        Some(list_items)
    } else {
        None
    }
}

/// Clean a list of tags: lowercase, trim, deduplicate, filter empties.
fn clean_tags(tags: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    tags.into_iter()
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty() && t.len() < 50 && seen.insert(t.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Strategy 1: Direct JSON array ──

    #[test]
    fn parse_json_array() {
        let input = r#"["portrait", "fantasy", "dark lighting"]"#;
        let tags = parse_tags(input).unwrap();
        assert_eq!(tags, vec!["portrait", "fantasy", "dark lighting"]);
    }

    // ── Strategy 2: Think tags + JSON ──

    #[test]
    fn parse_with_think_blocks() {
        let input = r#"<think>
Let me analyze this image. I see a portrait with dark lighting...
</think>

["portrait", "dark lighting", "woman"]"#;
        let tags = parse_tags(input).unwrap();
        assert_eq!(tags, vec!["portrait", "dark lighting", "woman"]);
    }

    #[test]
    fn parse_with_incomplete_think_block() {
        let input = "<think>\nStill thinking...\n[\"portrait\", \"fantasy\"]";
        let result = parse_tags(input);
        assert!(result.is_err());
    }

    #[test]
    fn strip_think_tags_complete() {
        let input = "<think>reasoning</think>result";
        assert_eq!(strip_think_tags(input), "result");
    }

    #[test]
    fn strip_think_tags_incomplete() {
        let input = "<think>reasoning without close";
        assert_eq!(strip_think_tags(input), "");
    }

    #[test]
    fn strip_think_tags_multiple() {
        let input = "<think>first</think>middle<think>second</think>end";
        assert_eq!(strip_think_tags(input), "middleend");
    }

    // ── Strategy 3: JSON object with "tags" key ──

    #[test]
    fn parse_object_with_tags_key() {
        let input = r#"{"tags": ["portrait", "dark", "moody"]}"#;
        let tags = parse_tags(input).unwrap();
        assert_eq!(tags, vec!["portrait", "dark", "moody"]);
    }

    #[test]
    fn parse_think_then_object() {
        let input = r#"<think>Looking at this...</think>{"tags": ["cat", "cute", "indoor"]}"#;
        let tags = parse_tags(input).unwrap();
        assert_eq!(tags, vec!["cat", "cute", "indoor"]);
    }

    // ── Strategy 4: Markdown code blocks ──

    #[test]
    fn parse_markdown_code_block() {
        let input = "Here are the tags:\n\n```json\n[\"portrait\", \"fantasy\", \"oil painting\"]\n```";
        let tags = parse_tags(input).unwrap();
        assert_eq!(tags, vec!["portrait", "fantasy", "oil painting"]);
    }

    #[test]
    fn parse_think_then_code_block() {
        let input =
            "<think>\nAnalyzing...\n</think>\n\n```json\n[\"landscape\", \"sunset\"]\n```";
        let tags = parse_tags(input).unwrap();
        assert_eq!(tags, vec!["landscape", "sunset"]);
    }

    #[test]
    fn parse_code_block_with_object() {
        let input = "```json\n{\"tags\": [\"a\", \"b\"]}\n```";
        let tags = parse_tags(input).unwrap();
        assert_eq!(tags, vec!["a", "b"]);
    }

    // ── Strategy 5: Bracket matching ──

    #[test]
    fn parse_with_surrounding_text() {
        let input = r#"Here are the tags: ["cat", "cute", "indoor"]"#;
        let tags = parse_tags(input).unwrap();
        assert_eq!(tags, vec!["cat", "cute", "indoor"]);
    }

    #[test]
    fn parse_mixed_text_and_json() {
        let input = "I found these:\n[\"a\", \"b\"]\nHope that helps!";
        let tags = parse_tags(input).unwrap();
        assert_eq!(tags, vec!["a", "b"]);
    }

    // ── Strategy 6: List extraction ──

    #[test]
    fn parse_numbered_list() {
        let input = "1. portrait\n2. fantasy\n3. dark lighting";
        let tags = parse_tags(input).unwrap();
        assert_eq!(tags, vec!["portrait", "fantasy", "dark lighting"]);
    }

    #[test]
    fn parse_bulleted_list() {
        let input = "- portrait\n- fantasy\n- dark lighting";
        let tags = parse_tags(input).unwrap();
        assert_eq!(tags, vec!["portrait", "fantasy", "dark lighting"]);
    }

    #[test]
    fn parse_star_bulleted_list() {
        let input = "* cat\n* cute\n* fluffy";
        let tags = parse_tags(input).unwrap();
        assert_eq!(tags, vec!["cat", "cute", "fluffy"]);
    }

    // ── Strategy 7: Comma-separated fallback ──

    #[test]
    fn parse_comma_separated() {
        let input = "portrait, fantasy, dark lighting";
        let tags = parse_tags(input).unwrap();
        assert_eq!(tags, vec!["portrait", "fantasy", "dark lighting"]);
    }

    // ── Edge cases ──

    #[test]
    fn parse_empty_fails() {
        assert!(parse_tags("").is_err());
        assert!(parse_tags("   ").is_err());
    }

    #[test]
    fn parse_cleans_whitespace_and_case() {
        let input = r#"["  Portrait  ", " FANTASY ", "Dark Lighting"]"#;
        let tags = parse_tags(input).unwrap();
        assert_eq!(tags, vec!["portrait", "fantasy", "dark lighting"]);
    }

    #[test]
    fn parse_deduplicates() {
        let input = r#"["cat", "Cat", "CAT", "dog"]"#;
        let tags = parse_tags(input).unwrap();
        assert_eq!(tags, vec!["cat", "dog"]);
    }

    #[test]
    fn parse_filters_long_tags() {
        let input = format!(
            r#"["good", "{}"]"#,
            "x".repeat(60)
        );
        let tags = parse_tags(&input).unwrap();
        assert_eq!(tags, vec!["good"]);
    }

    #[test]
    fn clean_tags_filters_empty() {
        let tags = vec!["good".to_string(), "".to_string(), "  ".to_string()];
        let cleaned = clean_tags(tags);
        assert_eq!(cleaned, vec!["good"]);
    }
}
