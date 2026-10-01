use std::collections::BTreeSet;
use std::io::{self, Write};

use anyhow::{anyhow, Context, Result};

use crate::manifest::Manifest;
use crate::{cargo_check, cover, imports, scan, toml_writer, workspace};

pub struct ScanResult {
    pub layout: workspace::Layout,
    pub manifest: Manifest,
    pub referenced_paths: BTreeSet<String>,
    pub referenced_features: BTreeSet<String>,
}

pub fn scan_workspace(use_cargo_check_fallback: bool) -> Result<ScanResult> {
    try_scan_workspace(use_cargo_check_fallback)?
        .ok_or_else(|| anyhow!("no workspace member depends on `engage-il2cpp`. Run from within a project that uses it"))
}

pub fn try_scan_workspace(use_cargo_check_fallback: bool) -> Result<Option<ScanResult>> {
    let cwd = std::env::current_dir()?;

    let Some(layout) = workspace::try_detect(&cwd)? else {
        return Ok(None);
    };

    scan_layout(&layout, use_cargo_check_fallback).map(Some)
}

pub fn scan_all_layouts(use_cargo_check_fallback: bool) -> Result<Vec<ScanResult>> {
    let cwd = std::env::current_dir()?;
    let layouts = workspace::detect_all(&cwd)?;
    let mut out = Vec::with_capacity(layouts.len());

    for layout in layouts {
        out.push(scan_layout(&layout, use_cargo_check_fallback)?);
    }

    Ok(out)
}

fn scan_layout(layout: &workspace::Layout, use_cargo_check_fallback: bool) -> Result<ScanResult> {
    let manifest = Manifest::load(&layout.bindings_root.join("features.json"))?;

    let listed = toml_writer::read_features(&layout.features_manifest).unwrap_or_default();

    // Seed the enabled set from whatever Cargo.toml already lists.
    let mut enabled: BTreeSet<String> = BTreeSet::new();
    for f in &listed {
        enabled.extend(manifest.closure(f));
    }

    // Finding references is a fixpoint, not one pass. The hand-written wrappers
    // in the bindings (ext.rs) hide their references behind #[cfg(feature = "...")],
    // so a wrapper's references only become visible once that wrapper's feature is
    // on. Each pass turns on the features we found last pass, which can uncover
    // more, so we loop until the enabled set stops growing. Without this a single
    // `apply` only peels off the first layer and the build still reports the next
    // layer as missing (you'd have to run apply again).
    let bindings_src = layout.bindings_root.join("src");
    let mut referenced_paths;

    loop {
        let mut found = BTreeSet::new();

        for root in &layout.scan_roots {
            found.extend(scan::collect_paths(root, &manifest, &[], &[], &enabled)?);
        }

        if bindings_src.exists() {
            found.extend(scan::collect_paths(
                &bindings_src,
                &manifest,
                &["crate"],
                &["generated"],
                &enabled,
            )?);
        }

        // Turn on whatever this pass referenced and see if that grew the set. If it
        // didn't, this pass already saw everything reachable, so we're converged.
        let before = enabled.len();
        for f in scan::paths_to_features(&found, &manifest) {
            enabled.extend(manifest.closure(&f));
        }

        referenced_paths = found;

        if enabled.len() == before {
            break;
        }
    }

    if use_cargo_check_fallback {
        match cargo_check::collect_unresolved_paths(&layout.check_manifest, &manifest) {
            Ok(extra) => referenced_paths.extend(extra),
            Err(e) => eprintln!("warning: cargo-check fallback skipped: {e}"),
        }
    }

    let referenced_features = scan::paths_to_features(&referenced_paths, &manifest);

    Ok(ScanResult {
        layout: layout.clone(),
        manifest,
        referenced_paths,
        referenced_features,
    })
}

pub fn check() -> Result<()> {
    let scans = scan_all_layouts(true)?;

    if scans.is_empty() {
        return Err(anyhow!("no workspace member depends on `engage-il2cpp`. Run from within a project that uses it"));
    }

    let multi_crate = scans.len() > 1;
    let mut total_missing = 0usize;

    for scanned in &scans {
        let listed = toml_writer::read_features(&scanned.layout.features_manifest)?;
        let required = cover::minimum_cover(&scanned.referenced_features, &scanned.manifest);
        let (missing, stale) = diff_features(&listed, &required, &scanned.manifest);

        if multi_crate {
            println!("=== {}", scanned.layout.features_manifest.display());
        }

        let count = scanned.referenced_paths.len();
        println!("scanned {count} engage-il2cpp path reference{}", plural(count));

        if missing.is_empty() && stale.is_empty() {
            println!("Cargo.toml is in sync");

            if multi_crate {
                println!();
            }

            continue;
        }

        if !missing.is_empty() {
            println!("\nmissing ({}):", missing.len());

            for f in &missing {
                println!("  + {f}");
            }

            total_missing += missing.len();
        }

        if !stale.is_empty() {
            println!("\nstale ({}):", stale.len());

            for f in &stale {
                println!("  - {f}");
            }
        }

        if multi_crate {
            println!();
        }
    }

    if total_missing > 0 {
        std::process::exit(1);
    }

    Ok(())
}

pub fn apply(yes: bool) -> Result<()> {
    let scans = scan_all_layouts(true)?;

    if scans.is_empty() {
        return Err(anyhow!("no workspace member depends on `engage-il2cpp`. Run from within a project that uses it"));
    }

    let mut pending: Vec<(workspace::Layout, BTreeSet<String>, BTreeSet<String>)> = Vec::new();

    for scanned in scans {
        let listed = toml_writer::read_features(&scanned.layout.features_manifest)?;
        let required = cover::minimum_cover(&scanned.referenced_features, &scanned.manifest);
        let (missing, _) = diff_features(&listed, &required, &scanned.manifest);

        if !missing.is_empty() {
            pending.push((scanned.layout, listed, missing));
        }
    }

    if pending.is_empty() {
        println!("nothing to do, every Cargo.toml already covers every referenced path");
        return Ok(());
    }

    for (layout, _, missing) in &pending {
        println!("Will add {} feature{} to {}:", missing.len(), plural(missing.len()), layout.features_manifest.display());

        for f in missing {
            println!("  + {f}");
        }
    }

    if !yes && !confirm("Apply?")? {
        println!("aborted");
        return Ok(());
    }

    for (layout, listed, missing) in pending {
        let mut updated = listed.clone();
        updated.extend(missing);

        toml_writer::write_features(&layout.features_manifest, &updated)?;
        println!("updated {}", layout.features_manifest.display());
    }

    Ok(())
}

/// Emit a JSON array of every method on a disabled-feature trait/impl block,
/// for the VS Code extension to use as method completions on receivers whose
/// `IXMethods` trait isn't currently in scope. Output one entry per `fn`,
/// keyed by receiver class.
///
/// Each entry: `{ method, class, path, feature }`.
///
/// Generated bindings emit methods in two shapes:
///   - `pub trait I<X>Methods: I<X> { fn ... { ... } ... }` for normal classes.
///   - `#[::unity2::methods] impl<T> X<T> { pub fn ... ; ... }` for generic classes.
/// Both are scanned uniformly: after a `#[cfg(feature = "...")]` line, the
/// next declaration gives us the receiver class, then we brace-count to the
/// matching `}` and pull every `fn` inside.
pub fn list_disabled_methods(json: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;

    let Some(layout) = workspace::try_detect(&cwd)? else {
        if json {
            println!("[]");
        }
        return Ok(());
    };

    let manifest = Manifest::load(&layout.bindings_root.join("features.json"))?;
    let listed = toml_writer::read_features(&layout.features_manifest).unwrap_or_default();

    let mut enabled: BTreeSet<String> = BTreeSet::new();
    for f in &listed {
        enabled.extend(manifest.closure(f));
    }

    let bindings_src = layout.bindings_root.join("src").join("generated");
    let mut entries: Vec<MethodEntry> = Vec::new();

    for entry in walkdir::WalkDir::new(&bindings_src).into_iter().filter_map(|e| e.ok()) {
        if entry.path().extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }

        let raw = match std::fs::read_to_string(entry.path()) {
            Ok(s) => s,
            Err(_) => continue,
        };

        scan_methods_in_file(&raw, entry.path(), &bindings_src, &enabled, &manifest.crate_ident(), &mut entries);
    }

    entries.sort_by(|a, b| a.method.cmp(&b.method).then_with(|| a.class.cmp(&b.class)));

    if json {
        let value: Vec<serde_json::Value> = entries
            .iter()
            .map(|e| {
                serde_json::json!({
                    "method": e.method,
                    "class": e.class,
                    "path": e.path,
                    "feature": e.feature,
                    "returns": e.returns,
                    "args": e.args,
                    "disabled": e.disabled,
                })
            })
            .collect();
        println!("{}", serde_json::to_string(&value)?);
    } else {
        for e in entries {
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                e.method,
                e.class,
                e.path,
                e.feature,
                e.returns.as_deref().unwrap_or(""),
                e.args.join(","),
                if e.disabled { "disabled" } else { "enabled" },
            );
        }
    }

    Ok(())
}

struct MethodEntry {
    method: String,
    class: String,
    path: String,
    feature: String,
    /// Leaf name of the method's return type, when extractable from the
    /// signature. Lets the VS Code provider chain-infer receivers like
    /// `foo.get_game_object().get_t` — `get_game_object` returns
    /// `GameObject`, so the receiver of `.get_t` is `GameObject`.
    returns: Option<String>,
    /// Positional parameter names (excluding `self`) in source order.
    /// The VS Code provider uses these as snippet placeholders so an
    /// accepted completion lands as `try_set_up_down_icon(icon, text, …)`
    /// with tab-stops at each arg, not as a hollow `(  )` the user has to
    /// rebuild from scratch.
    args: Vec<String>,
    /// `true` when the method's gating feature is not in the transitive
    /// closure of the project's current Cargo.toml feature list. The TS
    /// provider uses this to decide whether to *suggest* the method (only
    /// disabled ones — rust-analyzer handles enabled ones natively) while
    /// still using enabled methods' return-types for chain inference.
    disabled: bool,
}

fn scan_methods_in_file(
    text: &str,
    file: &std::path::Path,
    bindings_src: &std::path::Path,
    enabled: &BTreeSet<String>,
    crate_ident: &str,
    out: &mut Vec<MethodEntry>,
) {
    let namespace = derive_namespace(file, bindings_src);

    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let Some(feat) = parse_cfg_feature(lines[i]) else {
            i += 1;
            continue;
        };

        let is_disabled = !enabled.contains(&feat);

        // Walk past attributes/comments/blank lines to find the declaration.
        let mut j = i + 1;
        while j < lines.len() {
            let l = lines[j].trim_start();
            if l.is_empty() || l.starts_with('#') || l.starts_with("//") {
                j += 1;
            } else {
                break;
            }
        }

        if j >= lines.len() {
            break;
        }

        let Some(class) = parse_class_from_decl(lines[j]) else {
            i = j;
            continue;
        };

        // Find the opening `{` (may be on the decl line or a later line)
        // and brace-count to the matching close.
        let Some((block_open_line, _)) = find_open_brace(&lines, j) else {
            i = j + 1;
            continue;
        };

        let block_close_line = match find_block_close(&lines, block_open_line) {
            Some(l) => l,
            None => {
                i = block_open_line + 1;
                continue;
            }
        };

        for k in block_open_line..=block_close_line {
            if let Some(method) = parse_fn_decl(lines[k]) {
                let returns = extract_return_leaf(&lines, k);
                let args = extract_arg_names(&lines, k);
                out.push(MethodEntry {
                    method,
                    class: class.clone(),
                    path: format!("{crate_ident}::{namespace}::{class}"),
                    feature: feat.clone(),
                    returns,
                    args,
                    disabled: is_disabled,
                });
            }
        }

        i = block_close_line + 1;
    }
}

fn derive_namespace(file: &std::path::Path, bindings_src: &std::path::Path) -> String {
    let Ok(rel) = file.strip_prefix(bindings_src) else {
        return String::new();
    };

    let mut parts: Vec<String> = Vec::new();
    for comp in rel.components() {
        let std::path::Component::Normal(p) = comp else { continue };
        let s = p.to_string_lossy();
        let stem = s.strip_suffix(".rs").unwrap_or(&s);
        parts.push(stem.to_string());
    }

    parts.join("::")
}

fn parse_cfg_feature(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("#[cfg(feature = \"")?;
    let end = rest.find("\")]")?;
    Some(rest[..end].to_string())
}

fn parse_class_from_decl(line: &str) -> Option<String> {
    let trimmed = line.trim_start();

    if let Some(rest) = trimmed.strip_prefix("pub trait I") {
        if let Some(methods_pos) = rest.find("Methods") {
            let candidate = &rest[..methods_pos];

            if !candidate.is_empty() && candidate.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                let after = &rest[methods_pos + "Methods".len()..];
                let after_trim = after.trim_start();
                if after_trim.starts_with(':') || after_trim.starts_with('<') {
                    return Some(candidate.to_string());
                }
            }
        }
    }

    // `impl<...> X { ... }` or `impl<...> X<T...> { ... }`. Reject blanket
    // impl shape `impl<T> IXMethods for T {}` — its `for` would otherwise
    // make us grab `IXMethods` as the class.
    if let Some(rest) = trimmed.strip_prefix("impl") {
        if rest.contains(" for ") {
            return None;
        }

        let after_generics = strip_balanced_angles(rest).trim_start();
        let name: String = after_generics
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();

        if !name.is_empty() {
            return Some(name);
        }
    }

    None
}

fn strip_balanced_angles(s: &str) -> &str {
    let s = s.trim_start();

    if !s.starts_with('<') {
        return s;
    }

    let mut depth = 0i32;
    for (i, c) in s.char_indices() {
        if c == '<' {
            depth += 1;
        } else if c == '>' {
            depth -= 1;
            if depth == 0 {
                return &s[i + 1..];
            }
        }
    }

    s
}

fn find_open_brace(lines: &[&str], start: usize) -> Option<(usize, usize)> {
    for (offset, line) in lines.iter().enumerate().skip(start).take(8) {
        if let Some(col) = line.find('{') {
            return Some((offset, col));
        }
    }
    None
}

/// Find the closing `}` of a top-level `pub trait`/`impl` block.
///
/// Bindgen formats every top-level declaration with its closing `}` at
/// column 0, so we look for the next line starting with `}`. A naive
/// brace counter walks into trouble here — the generated panic strings
/// contain `{}` format specifiers (e.g. `"method lookup failed: {}::{}: {}"`),
/// which look like balanced brace pairs but throw off depth tracking on
/// any asymmetric appearance. Relying on the formatting invariant
/// sidesteps the entire mess.
fn find_block_close(lines: &[&str], open_line: usize) -> Option<usize> {
    for (offset, line) in lines.iter().enumerate().skip(open_line + 1) {
        if line.starts_with('}') {
            return Some(offset);
        }
    }
    None
}

/// Scan the lines starting at `fn_line` for the `-> Type` arrow, joining
/// at most a handful of continuation lines (the bindgen sometimes splits
/// long signatures across many lines), and return the leaf name of the
/// type. `()` collapses to None; primitives like `i32`/`bool` come back
/// as themselves.
fn extract_return_leaf(lines: &[&str], fn_line: usize) -> Option<String> {
    let max_scan = (fn_line + 12).min(lines.len());
    let mut joined = String::new();

    for k in fn_line..max_scan {
        joined.push_str(lines[k]);
        joined.push(' ');

        let after_arrow_search = joined.find("->");
        if after_arrow_search.is_some() {
            // Once we have the arrow, keep accumulating until we see the
            // closing `{` or `;`.
            if lines[k].contains('{') || lines[k].contains(';') {
                break;
            }
        } else if lines[k].contains('{') || lines[k].contains(';') {
            // Arrow never appears => `fn name(args) {`, return is `()`.
            return None;
        }
    }

    let arrow_pos = joined.find("->")?;
    let after_arrow = joined[arrow_pos + 2..].trim_start();

    // Stop at the body-open or stmt-end.
    let end = after_arrow
        .find(|c: char| c == '{' || c == ';')
        .unwrap_or(after_arrow.len());
    let type_expr = after_arrow[..end].trim();

    if type_expr.is_empty() || type_expr == "()" {
        return None;
    }

    // Strip a trailing `<...>` generics span if present, then take the
    // identifier that immediately precedes it (or the bare path's leaf
    // segment).
    let base = strip_trailing_generics(type_expr);
    let last = base.rsplit("::").next()?.trim();
    let leaf: String = last
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();

    if leaf.is_empty() {
        None
    } else {
        Some(leaf)
    }
}

/// Pull positional parameter names out of a method declaration that may
/// span multiple lines. Strips `self`/`&self`/`mut self` and skips through
/// balanced `(...)` and `<...>` so type expressions like
/// `impl Into<HashMap<K, V>>` don't break the comma split.
fn extract_arg_names(lines: &[&str], fn_line: usize) -> Vec<String> {
    let max_scan = (fn_line + 30).min(lines.len());
    let mut joined = String::new();

    for k in fn_line..max_scan {
        joined.push_str(lines[k]);
        joined.push('\n');

        // Stop once we've seen the full param list — heuristic: a line
        // ending in `{` or `;` after a `)` appears.
        if (lines[k].contains("->") || lines[k].contains('{') || lines[k].contains(';'))
            && (lines[k].contains(')') || joined.matches(')').count() > 0)
        {
            break;
        }
    }

    let fn_pos = match joined.find("fn ") {
        Some(p) => p,
        None => return Vec::new(),
    };

    let bytes = joined.as_bytes();
    let mut idx = fn_pos + 3;

    // Skip the method name + optional method-level generics, land on `(`.
    let after_fn = &joined[idx..];
    let name_len: usize = after_fn
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .map(|c| c.len_utf8())
        .sum();
    idx += name_len;

    while idx < bytes.len() && (bytes[idx] as char).is_whitespace() {
        idx += 1;
    }

    if idx < bytes.len() && bytes[idx] == b'<' {
        // skip balanced angles
        let mut depth = 0i32;
        while idx < bytes.len() {
            match bytes[idx] {
                b'<' => depth += 1,
                b'>' => {
                    depth -= 1;
                    if depth == 0 {
                        idx += 1;
                        break;
                    }
                }
                _ => {}
            }
            idx += 1;
        }
    }

    while idx < bytes.len() && (bytes[idx] as char).is_whitespace() {
        idx += 1;
    }

    if idx >= bytes.len() || bytes[idx] != b'(' {
        return Vec::new();
    }

    idx += 1;
    let paren_start = idx;

    let mut paren_depth = 1i32;
    let mut angle_depth = 0i32;

    while idx < bytes.len() && paren_depth > 0 {
        match bytes[idx] {
            b'(' if angle_depth == 0 => paren_depth += 1,
            b')' if angle_depth == 0 => {
                paren_depth -= 1;
                if paren_depth == 0 {
                    break;
                }
            }
            b'<' if paren_depth > 0 => angle_depth += 1,
            b'>' if angle_depth > 0 => angle_depth -= 1,
            _ => {}
        }
        idx += 1;
    }

    if paren_depth != 0 {
        return Vec::new();
    }

    let params_text = &joined[paren_start..idx];

    let mut out = Vec::new();

    for part in split_top_level(params_text) {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Skip `self`, `&self`, `&mut self`, `mut self`, `self: Foo`.
        if matches!(trimmed, "self" | "&self" | "&mut self" | "mut self") {
            continue;
        }
        if trimmed.starts_with("self") && trimmed[4..].trim_start().starts_with(':') {
            continue;
        }

        let colon = trimmed.find(':');
        let name_slice = match colon {
            Some(p) => &trimmed[..p],
            None => trimmed,
        };

        let name: String = name_slice
            .trim()
            .trim_start_matches("mut ")
            .trim_start_matches('&')
            .trim_start_matches("r#")
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();

        if !name.is_empty() {
            out.push(name);
        }
    }

    out
}

/// Comma-split that respects top-level only (skips commas inside `(...)`,
/// `<...>`, `[...]`).
fn split_top_level(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth_paren = 0i32;
    let mut depth_angle = 0i32;
    let mut depth_bracket = 0i32;
    let mut start = 0;

    for (i, c) in s.char_indices() {
        match c {
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            '<' => depth_angle += 1,
            '>' => depth_angle -= 1,
            '[' => depth_bracket += 1,
            ']' => depth_bracket -= 1,
            ',' if depth_paren == 0 && depth_angle == 0 && depth_bracket == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }

    parts.push(&s[start..]);
    parts
}

fn strip_trailing_generics(s: &str) -> &str {
    let s = s.trim_end();

    if !s.ends_with('>') {
        return s;
    }

    let mut depth = 0i32;
    let bytes = s.as_bytes();
    let mut i = bytes.len();

    while i > 0 {
        i -= 1;
        let c = bytes[i] as char;

        if c == '>' {
            depth += 1;
        } else if c == '<' {
            depth -= 1;

            if depth == 0 {
                return s[..i].trim_end();
            }
        }
    }

    s
}

fn parse_fn_decl(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let after_pub = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
    let rest = after_pub.strip_prefix("fn ")?;

    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();

    if name.is_empty() {
        return None;
    }

    let after_name = &rest[name.len()..];
    let after_name = after_name.trim_start();

    // Skip method-level generics if any.
    let after_generics = strip_balanced_angles(after_name);

    if !after_generics.trim_start().starts_with('(') {
        return None;
    }

    Some(name)
}

/// Emit a JSON array of every engage-il2cpp type whose gating feature is
/// NOT in the transitive closure of the current Cargo.toml feature list,
/// for the VS Code extension to surface as completion suggestions for
/// disabled features. Items are sorted by leaf name for determinism.
///
/// Output shape (one element per available path):
///   `[{ "path": "...", "feature": "...", "leaf": "..." }, ...]`
pub fn list_disabled(json: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;

    let Some(layout) = workspace::try_detect(&cwd)? else {
        if json {
            println!("[]");
        }
        return Ok(());
    };

    let manifest = Manifest::load(&layout.bindings_root.join("features.json"))?;
    let listed = toml_writer::read_features(&layout.features_manifest).unwrap_or_default();

    let mut enabled: BTreeSet<String> = BTreeSet::new();
    for f in &listed {
        enabled.extend(manifest.closure(f));
    }

    let mut entries: Vec<(&String, &String, &str)> = manifest
        .paths
        .iter()
        .filter(|(_, feature)| !enabled.contains(feature.as_str()))
        .map(|(path, feature)| {
            let leaf = path.rsplit("::").next().unwrap_or(path.as_str());
            (path, feature, leaf)
        })
        .collect();

    entries.sort_by(|a, b| a.2.cmp(b.2).then_with(|| a.0.cmp(b.0)));

    if json {
        let value: Vec<serde_json::Value> = entries
            .iter()
            .map(|(path, feature, leaf)| {
                serde_json::json!({
                    "path": path,
                    "feature": feature,
                    "leaf": leaf,
                })
            })
            .collect();
        println!("{}", serde_json::to_string(&value)?);
    } else {
        for (path, feature, _) in entries {
            println!("{path}\t{feature}");
        }
    }

    Ok(())
}

/// Map a rustc-flavored receiver path to the engage-il2cpp methods feature(s)
/// that gate the receiver's I*Methods trait. Used by the VS Code extension
/// to turn an `E0599 no method named X found for struct Y` diagnostic into
/// a "enable feature Y" quick-fix. Output: one feature name per line on
/// stdout, empty if no match.
///
/// The receiver string comes from rustc and may include `generated::`,
/// `__types::`, `r#` prefixes, and other noise; we normalize before
/// looking it up. If the full path doesn't match, we fall back to leaf-name
/// matching (last segment) and emit every candidate — the extension can
/// disambiguate or offer all of them.
pub fn for_receiver(receiver: &str) -> Result<()> {
    let cwd = std::env::current_dir()?;

    let Some(layout) = workspace::try_detect(&cwd)? else {
        return Ok(());
    };

    let manifest = Manifest::load(&layout.bindings_root.join("features.json"))?;

    let normalized = normalize_receiver(receiver);

    if let Some(feature) = manifest.paths.get(&normalized) {
        println!("{feature}");
        return Ok(());
    }

    let leaf = normalized.split("::").last().unwrap_or(&normalized);

    let mut features: BTreeSet<&String> = BTreeSet::new();
    for (_, feat) in manifest.paths_with_leaf(leaf) {
        features.insert(feat);
    }

    for f in features {
        println!("{f}");
    }

    Ok(())
}

fn normalize_receiver(s: &str) -> String {
    s.replace("r#", "")
        .replace("::generated::", "::")
        .replace("::__types::", "::")
}

pub fn explain(path: &str) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let layout = workspace::detect(&cwd)?;
    let manifest = Manifest::load(&layout.bindings_root.join("features.json"))?;

    let normalized = path.replace("r#", "");

    let Some(feature) = manifest.paths.get(&normalized) else {
        return Err(anyhow!(
            "no path `{path}` in features.json. The type may be from unity2 or filtered out of the bindings"
        ));
    };

    println!("{path}");
    println!("  feature: {feature}");

    let closure = manifest.closure(feature);
    let mut deps: Vec<&String> = closure.iter().filter(|f| f.as_str() != feature).collect();
    deps.sort();

    if !deps.is_empty() {
        println!("  enabling `{feature}` transitively activates {} features:", deps.len());

        for d in deps.iter().take(20) {
            println!("    - {d}");
        }

        if deps.len() > 20 {
            println!("    ... and {} more", deps.len() - 20);
        }
    }

    Ok(())
}

pub fn prune(yes: bool) -> Result<()> {
    let scans = scan_all_layouts(true)?;

    if scans.is_empty() {
        return Err(anyhow!("no workspace member depends on `engage-il2cpp`. Run from within a project that uses it"));
    }

    let mut dead_imports: Vec<imports::DeadImport> = Vec::new();
    let mut edits_by_file: std::collections::BTreeMap<std::path::PathBuf, Vec<imports::UseEdit>> = std::collections::BTreeMap::new();
    let mut pending_features: Vec<(workspace::Layout, BTreeSet<String>, BTreeSet<String>)> = Vec::new();

    for scanned in &scans {
        let findings = imports::find_dead_imports(&scanned.layout.scan_roots, &scanned.manifest)?;

        let dead_paths: BTreeSet<String> = findings.reports.iter().map(|d| d.path.clone()).collect();
        let effective_paths: BTreeSet<String> = scanned.referenced_paths.difference(&dead_paths).cloned().collect();
        let effective_features = scan::paths_to_features(&effective_paths, &scanned.manifest);

        let listed = toml_writer::read_features(&scanned.layout.features_manifest)?;
        let required = cover::minimum_cover(&effective_features, &scanned.manifest);
        let (_, stale) = diff_features(&listed, &required, &scanned.manifest);

        dead_imports.extend(findings.reports);
        for (file, edits) in findings.edits_by_file {
            edits_by_file.entry(file).or_default().extend(edits);
        }

        if !stale.is_empty() {
            pending_features.push((scanned.layout.clone(), listed, stale));
        }
    }

    if dead_imports.is_empty() && pending_features.is_empty() {
        println!("nothing to prune, every listed feature covers something referenced");
        return Ok(());
    }

    if !dead_imports.is_empty() {
        println!("Will remove {} unused use statement{}:", dead_imports.len(), plural(dead_imports.len()));

        for d in &dead_imports {
            println!("  - {} ({}:{})", d.ident, d.file.display(), d.start_line + 1);
        }
    }

    for (layout, _, stale) in &pending_features {
        if !dead_imports.is_empty() {
            println!();
        }

        println!("Will remove {} stale feature{} from {}:", stale.len(), plural(stale.len()), layout.features_manifest.display());

        for f in stale {
            println!("  - {f}");
        }
    }

    if !yes && !confirm("Apply?")? {
        println!("aborted");
        return Ok(());
    }

    for (file, edits) in &edits_by_file {
        imports::apply_use_edits(file, edits)?;
        println!("cleaned {}", file.display());
    }

    for (layout, listed, stale) in pending_features {
        let mut updated = listed.clone();

        for f in &stale {
            updated.remove(f);
        }

        toml_writer::write_features(&layout.features_manifest, &updated)?;
        println!("updated {}", layout.features_manifest.display());
    }

    Ok(())
}

pub fn add_feature(name: &str) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let layouts = workspace::detect_all(&cwd)?;

    if layouts.is_empty() {
        return Err(anyhow!("no workspace member depends on `engage-il2cpp`. Run from within a project that uses it"));
    }

    let manifest_path = layouts[0].bindings_root.join("features.json");
    let manifest = Manifest::load(&manifest_path)?;

    if !manifest.dependencies.contains_key(name) {
        return Err(anyhow!("`{name}` is not a known feature in {}", manifest_path.display()));
    }

    let feature_provides_path = manifest.paths.values().any(|f| f == name);

    if !feature_provides_path && !name.contains("-") {
        return Err(anyhow!("`{name}` does not match any known feature shape"));
    }

    for layout in layouts {
        let listed = toml_writer::read_features(&layout.features_manifest)?;

        if listed.contains(name) {
            continue;
        }

        let mut updated = listed;
        updated.insert(name.to_string());

        toml_writer::write_features(&layout.features_manifest, &updated)?;
        println!("added `{name}` to {}", layout.features_manifest.display());

        return Ok(());
    }

    println!("`{name}` already listed in every Cargo.toml");

    Ok(())
}

pub fn diff_features(listed: &BTreeSet<String>, required: &BTreeSet<String>, manifest: &Manifest) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut covered: BTreeSet<String> = BTreeSet::new();
    let mut stale: BTreeSet<String> = BTreeSet::new();

    for f in listed {
        let closure = manifest.closure(f);

        if closure.is_disjoint(required) {
            stale.insert(f.clone());
        } else {
            covered.extend(closure.intersection(required).cloned());
        }
    }

    let missing: BTreeSet<String> = required.difference(&covered).cloned().collect();

    (missing, stale)
}

fn confirm(prompt: &str) -> Result<bool> {
    print!("{prompt} [y/N] ");
    io::stdout().flush().context("flushing stdout")?;

    let mut buf = String::new();
    io::stdin().read_line(&mut buf).context("reading stdin")?;

    Ok(matches!(buf.trim().to_ascii_lowercase().as_str(), "y" | "yes"))
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}
