use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    #[serde(rename = "crate")]
    pub crate_name: String,
    #[serde(default)]
    pub import: Option<String>,
    #[allow(dead_code)]
    pub version: String,
    pub paths: BTreeMap<String, String>,
    pub dependencies: BTreeMap<String, Vec<String>>,
    #[serde(skip)]
    pub ext_wrappers: BTreeMap<String, Vec<String>>,
    /// Short re-export path -> canonical path. Types live at
    /// `engage::<ns>::<module>::<Type>` but the generated `<ns>` module
    /// re-exports them one level up (the `pub use <module>::{Type}` lines in
    /// app.rs etc), so code usually imports the short `engage::<ns>::<Type>`.
    /// This maps that short form back so the scanner recognizes it.
    #[serde(skip)]
    pub reexports: BTreeMap<String, String>,
}

const SCHEMA_VERSION: u32 = 1;

impl Manifest {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path).with_context(|| format!("reading manifest at {}", path.display()))?;
        let mut m: Manifest = serde_json::from_str(&raw).with_context(|| format!("parsing manifest at {}", path.display()))?;

        if m.schema_version != SCHEMA_VERSION {
            return Err(anyhow!(
                "manifest at {} is schema_version {}, but this CLI expects {}",
                path.display(),
                m.schema_version,
                SCHEMA_VERSION
            ));
        }

        if let Some(bindings_root) = path.parent() {
            m.ext_wrappers = load_ext_wrappers(bindings_root).unwrap_or_default();
        }

        m.reexports = build_reexports(&m.paths);

        Ok(m)
    }

    pub fn crate_ident(&self) -> String {
        self.import.as_ref().unwrap_or(&self.crate_name).replace('-', "_")
    }

    pub fn paths_with_leaf(&self, leaf: &str) -> Vec<(&String, &String)> {
        self.paths.iter().filter(|(path, _)| path.split("::").last() == Some(leaf)).collect()
    }

    /// Resolve a path as written in code to its (canonical path, gating feature).
    /// Accepts the canonical path itself (engage::app::fade::Fade) or the
    /// short re-export form code usually writes (engage::app::Fade), and
    /// always hands back the canonical one so callers agree on a single key.
    /// Returns None if the path names no known engage type. This is the one place
    /// that knows about re-exports, so both the scanner and the prune import check
    /// go through it instead of each reimplementing the lookup.
    pub fn resolve_path(&self, path: &str) -> Option<(&str, &str)> {
        if let Some((canonical, feature)) = self.paths.get_key_value(path) {
            return Some((canonical.as_str(), feature.as_str()));
        }

        let canonical = self.reexports.get(path)?;
        let feature = self.paths.get(canonical)?;
        Some((canonical.as_str(), feature.as_str()))
    }

    pub fn closure(&self, feature: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut stack = vec![feature.to_string()];

        while let Some(f) = stack.pop() {
            if !out.insert(f.clone()) {
                continue;
            }

            if let Some(deps) = self.dependencies.get(&f) {
                for d in deps {
                    if !out.contains(d) {
                        stack.push(d.clone());
                    }
                }
            }
        }

        out
    }
}

/// Invert the namespace-root re-exports: for each canonical
/// `engage::<ns>::<module>::<Type>`, the `<ns>` module re-exports it as
/// `engage::<ns>::<Type>`, so drop the module segment (second to last) to
/// get the short form code actually imports. A real `pub use` at one level can't
/// name two types the same way, so if two canonicals ever collapse to the same
/// short form we drop it rather than guess.
fn build_reexports(paths: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut grouped: BTreeMap<String, Vec<&String>> = BTreeMap::new();

    for canonical in paths.keys() {
        let segs: Vec<&str> = canonical.split("::").collect();

        // Need at least crate::ns::module::Type for there to be a module to drop.
        if segs.len() < 4 {
            continue;
        }

        let mut short_segs = segs.clone();
        short_segs.remove(segs.len() - 2);
        let short = short_segs.join("::");

        if short != *canonical {
            grouped.entry(short).or_default().push(canonical);
        }
    }

    grouped
        .into_iter()
        .filter_map(|(short, canonicals)| match canonicals.as_slice() {
            [only] => Some((short, (*only).clone())),
            _ => None,
        })
        .collect()
}

fn load_ext_wrappers(bindings_root: &Path) -> Option<BTreeMap<String, Vec<String>>> {
    let ext_path = bindings_root.join("src").join("ext.rs");
    let text = std::fs::read_to_string(&ext_path).ok()?;
    let file = syn::parse_file(&text).ok()?;

    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for item in &file.items {
        let syn::Item::Mod(m) = item else { continue };
        let features = extract_cfg_features(&m.attrs);
        if features.is_empty() {
            continue;
        }

        let Some((_, inner_items)) = m.content.as_ref() else { continue };

        for inner in inner_items {
            let (name, is_pub) = match inner {
                syn::Item::Struct(s) => (s.ident.to_string(), is_public(&s.vis)),
                syn::Item::Trait(t) => (t.ident.to_string(), is_public(&t.vis)),
                syn::Item::Fn(f) => (f.sig.ident.to_string(), is_public(&f.vis)),
                _ => continue,
            };

            if !is_pub {
                continue;
            }

            out.insert(name, features.clone());
        }
    }

    Some(out)
}

fn is_public(vis: &syn::Visibility) -> bool {
    matches!(vis, syn::Visibility::Public(_))
}

fn extract_cfg_features(attrs: &[syn::Attribute]) -> Vec<String> {
    for attr in attrs {
        if !attr.path().is_ident("cfg") {
            continue;
        }
        if let Ok(meta) = attr.parse_args::<syn::Meta>() {
            return collect_features(&meta);
        }
    }
    Vec::new()
}

fn collect_features(meta: &syn::Meta) -> Vec<String> {
    match meta {
        syn::Meta::NameValue(nv) if nv.path.is_ident("feature") => {
            if let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) = &nv.value
            {
                vec![s.value()]
            } else {
                Vec::new()
            }
        }
        syn::Meta::List(list) if list.path.is_ident("all") => {
            let parsed: syn::punctuated::Punctuated<syn::Meta, syn::Token![,]> = list
                .parse_args_with(syn::punctuated::Punctuated::parse_terminated)
                .unwrap_or_default();
            let mut out = Vec::new();
            for inner in parsed {
                out.extend(collect_features(&inner));
            }
            out
        }
        _ => Vec::new(),
    }
}
