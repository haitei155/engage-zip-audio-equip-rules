use std::collections::BTreeSet;

use proc_macro2::LineColumn;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Item, UseTree};

use crate::manifest::Manifest;

pub struct UnresolvedIdent {
    pub ident: String,
    pub feature: String,
    pub import_path: String,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub use_insert_line: u32,
}

pub fn find_unresolved(text: &str, manifest: &Manifest) -> Vec<UnresolvedIdent> {
    let file = match syn::parse_file(text) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };

    let in_scope = collect_in_scope(&file);
    let use_insert_line = next_line_after_last_use(&file);

    let mut visitor = BareIdentVisitor {
        in_scope: &in_scope,
        manifest,
        use_insert_line,
        found: Vec::new(),
        seen: BTreeSet::new(),
    };

    visitor.visit_file(&file);
    visitor.found
}

fn collect_in_scope(file: &syn::File) -> BTreeSet<String> {
    let mut out = BTreeSet::new();

    for item in &file.items {
        match item {
            Item::Use(u) => walk_use_tree(&u.tree, &mut out),
            Item::Struct(s) => {
                out.insert(s.ident.to_string());
            },
            Item::Enum(e) => {
                out.insert(e.ident.to_string());
            },
            Item::Fn(f) => {
                out.insert(f.sig.ident.to_string());
            },
            Item::Const(c) => {
                out.insert(c.ident.to_string());
            },
            Item::Static(s) => {
                out.insert(s.ident.to_string());
            },
            Item::Type(t) => {
                out.insert(t.ident.to_string());
            },
            Item::Mod(m) => {
                out.insert(m.ident.to_string());
            },
            Item::Trait(t) => {
                out.insert(t.ident.to_string());
            },
            Item::TraitAlias(t) => {
                out.insert(t.ident.to_string());
            },
            Item::Union(u) => {
                out.insert(u.ident.to_string());
            },
            Item::ExternCrate(e) => {
                out.insert(e.ident.to_string());
            },
            _ => {},
        }
    }

    out
}

fn walk_use_tree(tree: &UseTree, out: &mut BTreeSet<String>) {
    match tree {
        UseTree::Path(p) => walk_use_tree(&p.tree, out),
        UseTree::Name(n) => {
            out.insert(strip_raw(&n.ident.to_string()));
        },
        UseTree::Rename(r) => {
            out.insert(strip_raw(&r.rename.to_string()));
        },
        UseTree::Group(g) => {
            for item in &g.items {
                walk_use_tree(item, out);
            }
        },
        UseTree::Glob(_) => {},
    }
}

fn strip_raw(s: &str) -> String {
    s.strip_prefix("r#").unwrap_or(s).to_string()
}

fn next_line_after_last_use(file: &syn::File) -> u32 {
    let last_use_end_line = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Use(u) => Some(u.span().end().line),
            _ => None,
        })
        .max();

    match last_use_end_line {
        Some(line) => line as u32,
        None => 0,
    }
}

struct BareIdentVisitor<'a> {
    in_scope: &'a BTreeSet<String>,
    manifest: &'a Manifest,
    use_insert_line: u32,
    found: Vec<UnresolvedIdent>,
    seen: BTreeSet<(String, u32, u32)>,
}

impl<'a, 'ast> Visit<'ast> for BareIdentVisitor<'a> {
    fn visit_item(&mut self, item: &'ast Item) {
        if matches!(item, Item::Use(_)) {
            return;
        }

        syn::visit::visit_item(self, item);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        if path.leading_colon.is_some() {
            syn::visit::visit_path(self, path);
            return;
        }

        let Some(first) = path.segments.first() else {
            return;
        };

        let ident = strip_raw(&first.ident.to_string());

        if self.in_scope.contains(&ident) {
            syn::visit::visit_path(self, path);
            return;
        }

        let candidates = self.manifest.paths_with_leaf(&ident);

        if candidates.len() != 1 {
            syn::visit::visit_path(self, path);
            return;
        }

        let (import_path, feature) = candidates[0];
        let span = first.span();
        let start = span.start();
        let end = span.end();

        let key = (ident.clone(), start.line as u32, start.column as u32);

        if self.seen.contains(&key) {
            syn::visit::visit_path(self, path);
            return;
        }

        self.seen.insert(key);

        self.found.push(UnresolvedIdent {
            ident,
            feature: feature.clone(),
            import_path: import_path.clone(),
            start_line: line_to_lsp(&start),
            start_col: start.column as u32,
            end_line: line_to_lsp(&end),
            end_col: end.column as u32,
            use_insert_line: self.use_insert_line,
        });

        syn::visit::visit_path(self, path);
    }
}

fn line_to_lsp(lc: &LineColumn) -> u32 {
    if lc.line == 0 {
        0
    } else {
        (lc.line - 1) as u32
    }
}
