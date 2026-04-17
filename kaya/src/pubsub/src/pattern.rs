//! Glob pattern matching for `PSUBSCRIBE` subscriptions.
//!
//! Supported metacharacters (parity with RESP3 Pub/Sub / Redis semantics):
//! * `*`  – matches zero or more arbitrary bytes, **including** the `.`
//!   separator. This matches the reference semantics: `news.*` matches
//!   both `news.sport` and `news.sport.football`.
//! * `?`  – matches exactly one arbitrary byte.
//! * `[abc]` – matches a single byte from the class; supports ranges
//!   such as `[a-z]`.
//! * `\`  – escapes the next metacharacter.
//!
//! The pattern is compiled into a small token vector once and re-used for
//! every `matches()` call. For typical channel patterns (single-digit
//! length, few metacharacters) this is considerably faster than pulling
//! in a full regex engine.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::error::PubSubError;

/// Compiled glob pattern.
///
/// Cloning is cheap (`Arc` under the hood).
#[derive(Debug, Clone)]
pub struct Pattern {
    raw: Arc<Vec<u8>>,
    tokens: Arc<Vec<Token>>,
}

#[derive(Debug, Clone)]
enum Token {
    /// Literal byte.
    Byte(u8),
    /// `?` — exactly one arbitrary byte.
    Any,
    /// `*` — zero or more arbitrary bytes.
    Star,
    /// `[...]` — character class. `negated` indicates `[^...]`.
    Class {
        ranges: Vec<(u8, u8)>,
        singles: Vec<u8>,
        negated: bool,
    },
}

impl Pattern {
    /// Compile a glob pattern. Returns an error only for malformed character
    /// classes (unclosed `[`), so common inputs never fail.
    pub fn compile(raw: &[u8]) -> Result<Self, PubSubError> {
        let mut tokens = Vec::with_capacity(raw.len());
        let mut i = 0;
        while i < raw.len() {
            let b = raw[i];
            match b {
                b'\\' => {
                    i += 1;
                    if i >= raw.len() {
                        // Trailing backslash: treat as literal '\'.
                        tokens.push(Token::Byte(b'\\'));
                        break;
                    }
                    tokens.push(Token::Byte(raw[i]));
                    i += 1;
                }
                b'*' => {
                    // Collapse consecutive stars.
                    if !matches!(tokens.last(), Some(Token::Star)) {
                        tokens.push(Token::Star);
                    }
                    i += 1;
                }
                b'?' => {
                    tokens.push(Token::Any);
                    i += 1;
                }
                b'[' => {
                    // Parse character class.
                    let start = i + 1;
                    let mut end = start;
                    let mut negated = false;
                    if end < raw.len() && raw[end] == b'^' {
                        negated = true;
                        end += 1;
                    }
                    let class_start = end;
                    while end < raw.len() && raw[end] != b']' {
                        end += 1;
                    }
                    if end >= raw.len() {
                        return Err(PubSubError::PatternInvalid(
                            "unclosed character class".into(),
                        ));
                    }
                    let body = &raw[class_start..end];
                    let (ranges, singles) = parse_class(body);
                    tokens.push(Token::Class {
                        ranges,
                        singles,
                        negated,
                    });
                    i = end + 1;
                }
                _ => {
                    tokens.push(Token::Byte(b));
                    i += 1;
                }
            }
        }

        Ok(Self {
            raw: Arc::new(raw.to_vec()),
            tokens: Arc::new(tokens),
        })
    }

    /// Access the original pattern bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.raw
    }

    /// Test whether the pattern matches `channel`.
    pub fn matches(&self, channel: &[u8]) -> bool {
        match_tokens(&self.tokens, 0, channel, 0)
    }
}

impl PartialEq for Pattern {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl Eq for Pattern {}

impl Hash for Pattern {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

fn parse_class(body: &[u8]) -> (Vec<(u8, u8)>, Vec<u8>) {
    let mut ranges = Vec::new();
    let mut singles = Vec::new();
    let mut i = 0;
    while i < body.len() {
        if i + 2 < body.len() && body[i + 1] == b'-' {
            ranges.push((body[i], body[i + 2]));
            i += 3;
        } else {
            singles.push(body[i]);
            i += 1;
        }
    }
    (ranges, singles)
}

/// Recursive matcher with backtracking on `*`. For realistic channel names
/// (< 256 bytes) and few stars, this is fast enough and avoids allocations.
fn match_tokens(tokens: &[Token], ti: usize, channel: &[u8], ci: usize) -> bool {
    if ti == tokens.len() {
        return ci == channel.len();
    }

    match &tokens[ti] {
        Token::Byte(b) => {
            if ci < channel.len() && channel[ci] == *b {
                match_tokens(tokens, ti + 1, channel, ci + 1)
            } else {
                false
            }
        }
        Token::Any => {
            if ci < channel.len() {
                match_tokens(tokens, ti + 1, channel, ci + 1)
            } else {
                false
            }
        }
        Token::Star => {
            // Try consuming 0..=remaining bytes. Start from 0 for greediness
            // off the subsequent token.
            // Fast path: if the remaining pattern is just more stars, match.
            if tokens[ti + 1..].iter().all(|t| matches!(t, Token::Star)) {
                return true;
            }
            let mut consume = 0;
            loop {
                if match_tokens(tokens, ti + 1, channel, ci + consume) {
                    return true;
                }
                if ci + consume >= channel.len() {
                    return false;
                }
                consume += 1;
            }
        }
        Token::Class {
            ranges,
            singles,
            negated,
        } => {
            if ci >= channel.len() {
                return false;
            }
            let b = channel[ci];
            let mut hit = singles.contains(&b);
            if !hit {
                for (lo, hi) in ranges {
                    if b >= *lo && b <= *hi {
                        hit = true;
                        break;
                    }
                }
            }
            if *negated {
                hit = !hit;
            }
            if hit {
                match_tokens(tokens, ti + 1, channel, ci + 1)
            } else {
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Pattern {
        Pattern::compile(s.as_bytes()).expect("compile")
    }

    #[test]
    fn star_matches_empty() {
        assert!(p("*").matches(b""));
        assert!(p("*").matches(b"anything"));
    }

    #[test]
    fn star_matches_dot_separator() {
        // RESP3 Pub/Sub parity: '*' spans '.'.
        assert!(p("news.*").matches(b"news.sport"));
        assert!(p("news.*").matches(b"news.sport.football"));
    }

    #[test]
    fn literal_mismatch() {
        assert!(!p("news.*").matches(b"weather.today"));
    }

    #[test]
    fn question_single_byte() {
        assert!(p("h?llo").matches(b"hello"));
        assert!(p("h?llo").matches(b"hallo"));
        assert!(!p("h?llo").matches(b"hllo"));
        assert!(!p("h?llo").matches(b"heello"));
    }

    #[test]
    fn class_matches_range() {
        assert!(p("[a-c]at").matches(b"bat"));
        assert!(!p("[a-c]at").matches(b"dat"));
    }

    #[test]
    fn escape_literal_star() {
        assert!(p("a\\*b").matches(b"a*b"));
        assert!(!p("a\\*b").matches(b"axxb"));
    }
}
