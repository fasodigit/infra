// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-admin — tenant_template : loads T*.sql.tmpl files and substitutes
// {{SCHEMA}} / {{AUDIT_SCHEMA}} placeholders before execution by sqlx.
//
// Template directory: INFRA/terroir/migrations/tenant-template/
// Runtime path resolved from env TERROIR_MIGRATIONS_DIR (default: relative
// to the binary for development, absolute in production container).
//
// Template execution order: files sorted lexicographically (T001 < T002 ...).
// Placeholders are simple string replacements — no template engine dependency.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::{debug, info};

/// Rendered SQL ready for execution (placeholder-substituted).
#[derive(Debug, Clone)]
pub struct RenderedTemplate {
    pub filename: String,
    pub sql: String,
}

/// Loads, sorts, and renders all T*.sql.tmpl files from `template_dir`.
/// Returns rendered templates in lexicographic order (T001 < T002 < T100...).
pub async fn load_and_render(
    template_dir: &Path,
    schema: &str,
    audit_schema: &str,
) -> Result<Vec<RenderedTemplate>> {
    // Collect directory entries synchronously — directory is small (< 20 files).
    let mut entries: Vec<PathBuf> = std::fs::read_dir(template_dir)
        .with_context(|| format!("read tenant-template dir: {}", template_dir.display()))?
        .filter_map(|e: Result<std::fs::DirEntry, _>| e.ok().map(|de| de.path()))
        .filter(|p: &PathBuf| {
            p.extension()
                .and_then(|e: &std::ffi::OsStr| e.to_str())
                .map(|s: &str| s == "tmpl")
                .unwrap_or(false)
                && p.file_name()
                    .and_then(|n: &std::ffi::OsStr| n.to_str())
                    .map(|n: &str| n.starts_with('T'))
                    .unwrap_or(false)
        })
        .collect();

    entries.sort();

    if entries.is_empty() {
        anyhow::bail!("no T*.sql.tmpl files found in {}", template_dir.display());
    }

    info!(
        count = entries.len(),
        dir = %template_dir.display(),
        "loading tenant migration templates"
    );

    let mut rendered = Vec::with_capacity(entries.len());
    for path in &entries {
        let raw = tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("read template: {}", path.display()))?;

        let sql = raw
            .replace("{{SCHEMA}}", schema)
            .replace("{{AUDIT_SCHEMA}}", audit_schema);

        let filename = path
            .file_name()
            .and_then(|n: &std::ffi::OsStr| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        debug!(template = %filename, "rendered");
        rendered.push(RenderedTemplate { filename, sql });
    }

    Ok(rendered)
}

/// Resolves the migrations/tenant-template directory.
/// Precedence:
///   1. env var TERROIR_MIGRATIONS_DIR (for production containers)
///   2. TERROIR_WORKSPACE_ROOT env var + /migrations/tenant-template
///   3. Path relative to the executable (dev: cargo run from workspace root)
pub fn resolve_template_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("TERROIR_MIGRATIONS_DIR") {
        return PathBuf::from(dir);
    }

    if let Ok(root) = std::env::var("TERROIR_WORKSPACE_ROOT") {
        return PathBuf::from(root).join("migrations/tenant-template");
    }

    // Fallback: resolve relative to the binary's directory for `cargo run`.
    // Binary lives at target/debug/terroir-admin; workspace root is 3 levels up.
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        // target/debug -> target -> workspace root
        let candidate = parent
            .parent()
            .and_then(|p: &Path| p.parent())
            .map(|root: &Path| root.join("migrations/tenant-template"));
        if let Some(p) = candidate
            && p.exists()
        {
            return p;
        }
    }

    // Last resort: current working directory (for integration tests).
    PathBuf::from("migrations/tenant-template")
}
