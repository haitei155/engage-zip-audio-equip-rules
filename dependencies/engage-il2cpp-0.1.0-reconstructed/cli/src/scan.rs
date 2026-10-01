use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;
use syn::visit::Visit;
use syn::{Item, UseTree};

use crate::manifest::Manifest;

pub fn collect_paths(
    src_root: &Path,
    manifest: &Manifest,
    crate_synonyms: &[&str],
    skip_dirs: &[&str],
    enabled_features: &BTreeSet<String>,
) -> Result<BTreeSet<String>> {
    let mut visitor = PathVisitor::new(manifest, crate_synonyms, enabled_features);

    for entry in walkdir::WalkDir::new(src_root)
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy();
                !skip_dirs.iter().any(|s| name == *s)
            } else {
                true
            }
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs"))
    {
        let raw = match std::fs::read_to_string(entry.path()) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let file = match syn::parse_file(&raw) {
            Ok(f) => f,
            Err(_) => continue,
        };

        visitor.visit_file(&file);
    }

    Ok(visitor.out)
}

pub fn paths_to_features(paths: &BTreeSet<String>, manifest: &Manifest) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = paths
        .iter()
        .filter_map(|p| manifest.paths.get(p))
        .cloned()
        .collect();

    for p in paths {
        let leaf = p.rsplit("::").next().unwrap_or(p);
        if let Some(feats) = manifest.ext_wrappers.get(leaf) {
            out.extend(feats.iter().cloned());
        }
    }

    out
}

struct PathVisitor<'a> {
    manifest: &'a Manifest,
    accepted_prefixes: Vec<String>,
    enabled_features: &'a BTreeSet<String>,
    out: BTreeSet<String>,
}

impl<'a> PathVisitor<'a> {
    fn new(
        manifest: &'a Manifest,
        synonyms: &[&str],
        enabled_features: &'a BTreeSet<String>,
    ) -> Self {
        let crate_ident = manifest.crate_ident();
        let mut accepted_prefixes = vec![crate_ident.clone()];

        for s in synonyms {
            accepted_prefixes.push(s.to_string());
        }

        Self {
            manifest,
            accepted_prefixes,
            enabled_features,
            out: BTreeSet::new(),
        }
    }

    fn canonicalize_first_segment(&self, segments: &mut Vec<String>) -> bool {
        let crate_ident = self.manifest.crate_ident();

        match segments.first() {
            Some(first) if self.accepted_prefixes.contains(first) => {
                segments[0] = crate_ident;
                true
            },
            _ => false,
        }
    }

    fn record(&mut self, mut segments: Vec<String>) {
        if !self.canonicalize_first_segment(&mut segments) {
            return;
        }

        for n in (2..=segments.len()).rev() {
            let candidate: String = segments[..n].join("::");

            // Matches either the canonical path or the short re-export form code
            // usually writes (engage::app::Fade). Record the canonical so it
            // maps to a feature downstream.
            if let Some((canonical, _)) = self.manifest.resolve_path(&candidate) {
                let canonical = canonical.to_string();
                self.out.insert(canonical);
                return;
            }

            if let Some(leaf) = segments.get(n - 1) {
                if self.manifest.ext_wrappers.contains_key(leaf) {
                    self.out.insert(candidate);
                    return;
                }
            }

            for rewritten in rewrite_synthetic_trait(&segments[..n]) {
                if self.manifest.paths.contains_key(&rewritten) {
                    self.out.insert(rewritten);
                    return;
                }
            }
        }
    }
}

fn rewrite_synthetic_trait(segments: &[String]) -> Vec<String> {
    if segments.len() < 2 {
        return Vec::new();
    }

    let mut out = Vec::new();
    let last = segments.last().unwrap();

    let class_name: Option<String> = last.strip_prefix('I').and_then(|inner| {
        if let Some(stripped) = inner.strip_suffix("Methods") {
            (!stripped.is_empty()).then(|| stripped.to_string())
        } else if !inner.is_empty() {
            Some(inner.to_string())
        } else {
            None
        }
    });

    let Some(name) = class_name else {
        return out;
    };

    out.push(replace_last_segment(segments, &name));

    let mut canonical: Vec<&str> = segments[..segments.len() - 1].iter().map(String::as_str).collect();
    let lowered = name.to_lowercase();
    canonical.push(&lowered);
    canonical.push(&name);
    out.push(canonical.join("::"));

    out
}

fn replace_last_segment(segments: &[String], replacement: &str) -> String {
    let mut out: Vec<&str> = segments[..segments.len() - 1].iter().map(String::as_str).collect();
    out.push(replacement);
    out.join("::")
}

impl<'a, 'ast> Visit<'ast> for PathVisitor<'a> {
    fn visit_item(&mut self, item: &'ast Item) {
        if let Some(attrs) = item_attrs(item) {
            if !cfg_satisfied(attrs, self.enabled_features) {
                return;
            }
        }

        if let Item::Use(use_item) = item {
            walk_use_tree(&use_item.tree, &mut Vec::new(), self);
        }

        syn::visit::visit_item(self, item);
    }

    fn visit_attribute(&mut self, attr: &'ast syn::Attribute) {
        if let Some(path) = unity2_attr_kind(&attr.meta.path()) {
            if let Some((ns, name)) = extract_il2cpp_pair(&attr.meta, path) {
                if let Some(candidate) = il2cpp_path_to_rust(&ns, &name, &self.manifest.crate_ident()) {
                    if self.manifest.paths.contains_key(&candidate) {
                        self.out.insert(candidate);
                    }
                }
            }
        }

        syn::visit::visit_attribute(self, attr);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        let segments: Vec<String> = path.segments.iter().map(|s| ident_no_raw(&s.ident.to_string())).collect();

        self.record(segments);
        syn::visit::visit_path(self, path);
    }
}

fn walk_use_tree(tree: &UseTree, prefix: &mut Vec<String>, visitor: &mut PathVisitor) {
    match tree {
        UseTree::Path(p) => {
            prefix.push(ident_no_raw(&p.ident.to_string()));
            walk_use_tree(&p.tree, prefix, visitor);
            prefix.pop();
        },
        UseTree::Name(n) => {
            prefix.push(ident_no_raw(&n.ident.to_string()));
            visitor.record(prefix.clone());
            prefix.pop();
        },
        UseTree::Rename(r) => {
            prefix.push(ident_no_raw(&r.ident.to_string()));
            visitor.record(prefix.clone());
            prefix.pop();
        },
        UseTree::Group(g) => {
            for item in &g.items {
                walk_use_tree(item, prefix, visitor);
            }
        },
        UseTree::Glob(_) => {
            record_glob(prefix, visitor);
        },
    }
}

fn record_glob(prefix: &[String], visitor: &mut PathVisitor) {
    let mut prefix = prefix.to_vec();

    if !visitor.canonicalize_first_segment(&mut prefix) {
        return;
    }

    let prefix_joined = prefix.join("::");
    let prefix_with_sep = format!("{prefix_joined}::");

    for path in visitor.manifest.paths.keys() {
        if path.starts_with(&prefix_with_sep) {
            let rest = &path[prefix_with_sep.len()..];

            if !rest.contains("::") {
                visitor.out.insert(path.clone());
            }
        }
    }
}

fn ident_no_raw(s: &str) -> String {
    s.strip_prefix("r#").unwrap_or(s).to_string()
}

fn unity2_attr_kind(path: &syn::Path) -> Option<Unity2AttrKind> {
    let last = path.segments.last()?.ident.to_string();

    match last.as_str() {
        "hook" | "from_offset" => Some(Unity2AttrKind::Positional),
        "class" => Some(Unity2AttrKind::Named),
        _ => None,
    }
}

#[derive(Copy, Clone)]
enum Unity2AttrKind {
    Positional,
    Named,
}

fn extract_il2cpp_pair(meta: &syn::Meta, kind: Unity2AttrKind) -> Option<(String, String)> {
    let list = match meta {
        syn::Meta::List(l) => l,
        _ => return None,
    };

    match kind {
        Unity2AttrKind::Positional => {
            let parsed: syn::punctuated::Punctuated<syn::Expr, syn::Token![,]> =
                list.parse_args_with(syn::punctuated::Punctuated::parse_terminated).ok()?;

            let mut strs = parsed.iter().filter_map(|e| {
                match e {
                    syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) => Some(s.value()),
                    _ => None,
                }
            });

            let ns = strs.next()?;
            let name = strs.next()?;

            Some((ns, name))
        },
        Unity2AttrKind::Named => {
            let parsed: syn::punctuated::Punctuated<syn::MetaNameValue, syn::Token![,]> =
                list.parse_args_with(syn::punctuated::Punctuated::parse_terminated).ok()?;

            let mut ns: Option<String> = None;
            let mut name: Option<String> = None;

            for item in parsed.iter() {
                let key = item.path.get_ident()?.to_string();

                let s = match &item.value {
                    syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) => s.value(),
                    _ => continue,
                };

                match key.as_str() {
                    "namespace" => ns = Some(s),
                    "name" => name = Some(s),
                    _ => {},
                }
            }

            Some((ns?, name?))
        },
    }
}

fn il2cpp_path_to_rust(namespace: &str, name: &str, crate_ident: &str) -> Option<String> {
    if name.is_empty() {
        return None;
    }

    let mut segments: Vec<String> = namespace.split('.').filter(|s| !s.is_empty()).map(str::to_lowercase).collect();

    if segments.is_empty() {
        return None;
    }

    let outer = name.split('.').next().unwrap();
    segments.push(outer.to_lowercase());

    let ident = name.replace('.', "_");
    segments.push(ident);

    let mut out = String::from(crate_ident);

    for seg in &segments {
        out.push_str("::");
        out.push_str(seg);
    }

    Some(out)
}

fn item_attrs(item: &Item) -> Option<&[syn::Attribute]> {
    match item {
        Item::Const(i) => Some(&i.attrs),
        Item::Enum(i) => Some(&i.attrs),
        Item::ExternCrate(i) => Some(&i.attrs),
        Item::Fn(i) => Some(&i.attrs),
        Item::ForeignMod(i) => Some(&i.attrs),
        Item::Impl(i) => Some(&i.attrs),
        Item::Macro(i) => Some(&i.attrs),
        Item::Mod(i) => Some(&i.attrs),
        Item::Static(i) => Some(&i.attrs),
        Item::Struct(i) => Some(&i.attrs),
        Item::Trait(i) => Some(&i.attrs),
        Item::TraitAlias(i) => Some(&i.attrs),
        Item::Type(i) => Some(&i.attrs),
        Item::Union(i) => Some(&i.attrs),
        Item::Use(i) => Some(&i.attrs),
        _ => None,
    }
}

fn cfg_satisfied(attrs: &[syn::Attribute], enabled: &BTreeSet<String>) -> bool {
    for attr in attrs {
        if !attr.path().is_ident("cfg") {
            continue;
        }
        let predicate: syn::Meta = match attr.parse_args() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !eval_cfg(&predicate, enabled) {
            return false;
        }
    }
    true
}

fn eval_cfg(meta: &syn::Meta, enabled: &BTreeSet<String>) -> bool {
    match meta {
        syn::Meta::Path(_) => true,
        syn::Meta::NameValue(nv) => {
            if !nv.path.is_ident("feature") {
                return true;
            }
            match &nv.value {
                syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) => {
                    enabled.contains(&s.value())
                },
                _ => true,
            }
        },
        syn::Meta::List(list) => {
            let parsed: syn::punctuated::Punctuated<syn::Meta, syn::Token![,]> =
                match list.parse_args_with(syn::punctuated::Punctuated::parse_terminated) {
                    Ok(p) => p,
                    Err(_) => return true,
                };
            if list.path.is_ident("all") {
                parsed.iter().all(|m| eval_cfg(m, enabled))
            } else if list.path.is_ident("any") {
                parsed.iter().any(|m| eval_cfg(m, enabled))
            } else if list.path.is_ident("not") {
                match parsed.iter().next() {
                    Some(m) => !eval_cfg(m, enabled),
                    None => true,
                }
            } else {
                true
            }
        },
    }
}
