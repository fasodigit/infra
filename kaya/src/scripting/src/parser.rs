//! Parser for the KAYA Functions library header and function declarations.
//!
//! A library source always starts with a shebang-like metadata line:
//!
//! ```text
//! #!rhai name=mylib engine=rhai
//! fn my_function(keys, args) { ... }
//! fn other(keys, args) { ... }
//! ```
//!
//! The parser is deliberately tolerant of extra whitespace and of
//! case-insensitive keys but rejects anything that does not fit the grammar.

use crate::error::FunctionError;
use crate::functions::{EngineKind, FunctionFlags, FunctionMeta};
use std::collections::HashMap;

/// Parsed header metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryHeader {
    pub name: String,
    pub engine: EngineKind,
}

/// Maximum length of a library name (keeps the index compact and avoids
/// pathological inputs).
pub const MAX_LIBRARY_NAME_LEN: usize = 64;

/// Parse the shebang metadata line on the first non-empty line of `src`.
///
/// Returns [`FunctionError::MissingShebang`] if the first line does not start
/// with `#!` and [`FunctionError::InvalidLibraryName`] if the name is missing
/// or contains characters outside `[a-zA-Z_][a-zA-Z0-9_]*`.
#[tracing::instrument(level = "trace", skip(src))]
pub fn parse_header(src: &str) -> Result<LibraryHeader, FunctionError> {
    let first = src
        .lines()
        .next()
        .ok_or(FunctionError::MissingShebang)?
        .trim();

    if !first.starts_with("#!") {
        return Err(FunctionError::MissingShebang);
    }

    // Strip "#!" then optional engine tag ("rhai", "wasm", "lua"). We accept
    // both `#!rhai name=...` and `#! name=... engine=rhai` forms.
    let rest = first.trim_start_matches("#!").trim();

    // Split on whitespace into `key=value` pairs (plus an optional leading
    // bare engine word).
    let mut name: Option<String> = None;
    let mut engine_word: Option<String> = None;
    let mut kv: HashMap<String, String> = HashMap::new();

    for (idx, tok) in rest.split_whitespace().enumerate() {
        if let Some((k, v)) = tok.split_once('=') {
            kv.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        } else if idx == 0 {
            engine_word = Some(tok.to_ascii_lowercase());
        } else {
            return Err(FunctionError::InvalidLibraryName(format!(
                "unexpected token '{tok}' in header"
            )));
        }
    }

    if let Some(raw) = kv.remove("name") {
        validate_name(&raw)?;
        name = Some(raw);
    }

    let name = name.ok_or_else(|| {
        FunctionError::InvalidLibraryName("missing required 'name=' metadata".into())
    })?;

    // Resolve engine: explicit engine=... wins, otherwise the leading word.
    let engine_raw = kv
        .remove("engine")
        .or(engine_word)
        .unwrap_or_else(|| "rhai".into());

    let engine = match engine_raw.as_str() {
        "rhai" => EngineKind::Rhai,
        "wasm" => EngineKind::Wasm,
        "lua" => EngineKind::LuaCompat,
        other => {
            return Err(FunctionError::InvalidLibraryName(format!(
                "unsupported engine '{other}'"
            )));
        }
    };

    Ok(LibraryHeader { name, engine })
}

/// Validate an identifier against `[a-zA-Z_][a-zA-Z0-9_]*` and the length cap.
pub fn validate_name(raw: &str) -> Result<(), FunctionError> {
    if raw.is_empty() || raw.len() > MAX_LIBRARY_NAME_LEN {
        return Err(FunctionError::InvalidLibraryName(format!(
            "length must be 1..={MAX_LIBRARY_NAME_LEN}, got {}",
            raw.len()
        )));
    }

    let mut chars = raw.chars();
    let first = chars.next().expect("length already checked");
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(FunctionError::InvalidLibraryName(format!(
            "first character must be a letter or '_', got '{first}'"
        )));
    }
    for c in chars {
        if !(c.is_ascii_alphanumeric() || c == '_') {
            return Err(FunctionError::InvalidLibraryName(format!(
                "invalid character '{c}' (allowed: [a-zA-Z0-9_])"
            )));
        }
    }
    Ok(())
}

/// Scan the source for top-level `fn <name>(keys, args) { ... }` declarations
/// and return the exported [`FunctionMeta`] entries.
///
/// Supported optional annotations live on the preceding line as a comment:
///
/// ```text
/// // @readonly
/// // @no-cluster
/// fn read_only_fn(keys, args) { ... }
/// ```
///
/// The parser is regex-free (keeps the dependency list small) and only
/// recognises top-level `fn` declarations (no leading whitespace).
#[tracing::instrument(level = "trace", skip(src))]
pub fn extract_functions(src: &str) -> Result<Vec<FunctionMeta>, FunctionError> {
    let mut out = Vec::new();
    let mut pending_flags = FunctionFlags::default();

    for line in src.lines() {
        let trimmed = line.trim_start();

        // Collect flag annotations on comment lines that precede a `fn` decl.
        if let Some(rest) = trimmed.strip_prefix("//") {
            let tag = rest.trim();
            match tag {
                "@readonly" => pending_flags.readonly = true,
                "@no-cluster" => pending_flags.no_cluster = true,
                "@allow-stale" => pending_flags.allow_stale = true,
                _ => {}
            }
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        if let Some(after_fn) = trimmed.strip_prefix("fn ") {
            if let Some((name, _rest)) = after_fn.split_once('(') {
                let name = name.trim();
                if name.is_empty() {
                    return Err(FunctionError::InvalidLibraryName(
                        "unnamed fn declaration".into(),
                    ));
                }
                validate_name(name)?;
                out.push(FunctionMeta {
                    name: name.to_string(),
                    flags: pending_flags,
                });
                pending_flags = FunctionFlags::default();
                continue;
            }
        }

        // Any non-comment, non-blank, non-fn line resets pending annotations.
        pending_flags = FunctionFlags::default();
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_header_rhai_explicit() {
        let hdr = parse_header("#!rhai name=lib1 engine=rhai\nfn a() {}").unwrap();
        assert_eq!(hdr.name, "lib1");
        assert!(matches!(hdr.engine, EngineKind::Rhai));
    }

    #[test]
    fn parse_header_missing_shebang() {
        assert!(matches!(
            parse_header("fn a() {}"),
            Err(FunctionError::MissingShebang)
        ));
    }

    #[test]
    fn parse_header_invalid_name() {
        assert!(matches!(
            parse_header("#!rhai name=1bad engine=rhai\n"),
            Err(FunctionError::InvalidLibraryName(_))
        ));
    }

    #[test]
    fn extract_annotated_functions() {
        let src = "#!rhai name=l engine=rhai\n// @readonly\nfn ro(keys, args) {}\nfn rw(keys, args) {}\n";
        let fns = extract_functions(src).unwrap();
        assert_eq!(fns.len(), 2);
        assert!(fns[0].flags.readonly);
        assert!(!fns[1].flags.readonly);
    }
}
