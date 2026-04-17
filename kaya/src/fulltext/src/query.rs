//! Query language translator: maps a subset RESP3-FT query syntax to
//! Tantivy's native query language and then parses it via
//! `tantivy::query::QueryParser`.
//!
//! ## Supported syntax
//!
//! | Input syntax             | Meaning                        |
//! |--------------------------|--------------------------------|
//! | `@field:word`            | term query on a specific field |
//! | `@field:"exact phrase"`  | phrase query                   |
//! | `@field:[10 20]`         | numeric range [10, 20]         |
//! | `@field:{tag1\|tag2}`    | tag filter (OR of tags)        |
//! | `-(word)`                | boolean NOT                    |
//! | `word OR word`           | boolean OR                     |
//! | `word AND word`          | boolean AND (also default)     |
//! | `word~2`                 | fuzzy term (edit distance 2)   |

use std::borrow::Cow;

/// Translate a RESP3-FT query string into a Tantivy query string.
///
/// The output is a string suitable for parsing with
/// `tantivy::query::QueryParser::parse_query`.
pub fn translate_to_tantivy(input: &str) -> Cow<'_, str> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Cow::Borrowed("*");
    }

    // Fast path: no FT-specific syntax — pass through directly.
    if !trimmed.contains('@') && !trimmed.contains("-(") {
        return Cow::Borrowed(trimmed);
    }

    let mut out = String::with_capacity(trimmed.len() + 16);
    let chars: Vec<char> = trimmed.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Negation: -(word)
        if chars[i] == '-' && i + 1 < len && chars[i + 1] == '(' {
            out.push_str("NOT (");
            i += 2; // skip '-' and '('
            // copy until matching ')'
            while i < len && chars[i] != ')' {
                out.push(chars[i]);
                i += 1;
            }
            out.push(')');
            if i < len { i += 1; } // consume ')'
            continue;
        }

        // Field query: @field:...
        if chars[i] == '@' {
            i += 1; // skip '@'
            // collect field name
            let mut field = String::new();
            while i < len && chars[i] != ':' {
                field.push(chars[i]);
                i += 1;
            }
            if i < len { i += 1; } // skip ':'

            if i >= len {
                break;
            }

            match chars[i] {
                // Phrase: @field:"exact phrase"
                '"' => {
                    i += 1;
                    out.push_str(&field);
                    out.push(':');
                    out.push('"');
                    while i < len && chars[i] != '"' {
                        out.push(chars[i]);
                        i += 1;
                    }
                    out.push('"');
                    if i < len { i += 1; } // closing '"'
                }
                // Numeric range: @field:[from to]
                '[' => {
                    i += 1;
                    let mut range_content = String::new();
                    while i < len && chars[i] != ']' {
                        range_content.push(chars[i]);
                        i += 1;
                    }
                    if i < len { i += 1; } // skip ']'

                    let parts: Vec<&str> = range_content.split_whitespace().collect();
                    if parts.len() == 2 {
                        // Tantivy range syntax: field:[from TO to]
                        out.push_str(&field);
                        out.push_str(":[");
                        out.push_str(parts[0]);
                        out.push_str(" TO ");
                        out.push_str(parts[1]);
                        out.push(']');
                    } else {
                        // fallback: emit as-is
                        out.push_str(&field);
                        out.push_str(":[");
                        out.push_str(&range_content);
                        out.push(']');
                    }
                }
                // Tag filter: @field:{tag1|tag2}
                '{' => {
                    i += 1;
                    let mut tag_content = String::new();
                    while i < len && chars[i] != '}' {
                        tag_content.push(chars[i]);
                        i += 1;
                    }
                    if i < len { i += 1; } // skip '}'

                    // Convert pipe-separated tags to OR clauses
                    let tags: Vec<&str> = tag_content.split('|').collect();
                    if tags.len() == 1 {
                        out.push_str(&field);
                        out.push(':');
                        out.push_str(tags[0].trim());
                    } else {
                        out.push('(');
                        for (ti, tag) in tags.iter().enumerate() {
                            if ti > 0 {
                                out.push_str(" OR ");
                            }
                            out.push_str(&field);
                            out.push(':');
                            out.push_str(tag.trim());
                        }
                        out.push(')');
                    }
                }
                // Simple term: @field:word
                _ => {
                    let mut term = String::new();
                    while i < len && !chars[i].is_whitespace() {
                        term.push(chars[i]);
                        i += 1;
                    }
                    // Fuzzy: word~N
                    if let Some(tilde_pos) = term.find('~') {
                        let (word, dist) = term.split_at(tilde_pos);
                        out.push_str(&field);
                        out.push(':');
                        out.push_str(word);
                        out.push_str(dist); // keeps ~2 etc.
                    } else {
                        out.push_str(&field);
                        out.push(':');
                        out.push_str(&term);
                    }
                }
            }
            continue;
        }

        // Pass-through for everything else (AND, OR, plain words, parens, fuzzy
        // terms with ~N at top level, etc.).
        out.push(chars[i]);
        i += 1;
    }

    Cow::Owned(out)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_word_passthrough() {
        assert_eq!(translate_to_tantivy("hello"), "hello");
    }

    #[test]
    fn field_term_translation() {
        let t = translate_to_tantivy("@title:rust");
        assert_eq!(t, "title:rust");
    }

    #[test]
    fn phrase_query_translation() {
        let t = translate_to_tantivy(r#"@body:"hello world""#);
        assert_eq!(t, r#"body:"hello world""#);
    }

    #[test]
    fn numeric_range_translation() {
        let t = translate_to_tantivy("@price:[10 100]");
        assert_eq!(t, "price:[10 TO 100]");
    }

    #[test]
    fn tag_pipe_translation() {
        let t = translate_to_tantivy("@category:{rust|go|java}");
        assert_eq!(t, "(category:rust OR category:go OR category:java)");
    }

    #[test]
    fn negation_translation() {
        let t = translate_to_tantivy("-(spam)");
        assert_eq!(t, "NOT (spam)");
    }

    #[test]
    fn fuzzy_at_field_level() {
        let t = translate_to_tantivy("@name:helloo~2");
        assert_eq!(t, "name:helloo~2");
    }

    #[test]
    fn empty_input_becomes_star() {
        assert_eq!(translate_to_tantivy(""), "*");
        assert_eq!(translate_to_tantivy("   "), "*");
    }
}
