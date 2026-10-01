use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Item, UseTree};
use walkdir::WalkDir;

use crate::manifest::Manifest;

pub struct DeadImport {
    pub file: PathBuf,
    pub start_line: u32,
    pub ident: String,
    pub path: String,
    pub feature: String,
}

pub enum UseEdit {
    DropItem { start_line: u32, end_line: u32 },
    Replace { start_line: u32, end_line: u32, new_text: String },
}

pub struct PruneFindings {
    pub reports: Vec<DeadImport>,
    pub edits_by_file: BTreeMap<PathBuf, Vec<UseEdit>>,
}

pub fn find_dead_imports(roots: &[PathBuf], manifest: &Manifest) -> Result<PruneFindings> {
    let mut reports = Vec::new();
    let mut edits_by_file: BTreeMap<PathBuf, Vec<UseEdit>> = BTreeMap::new();

    for root in roots {
        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            if entry.path().extension().and_then(|s| s.to_str()) != Some("rs") {
                continue;
            }

            let raw = match std::fs::read_to_string(entry.path()) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let (file_reports, file_edits) = process_file(&raw, manifest);

            for r in file_reports {
                reports.push(DeadImport {
                    file: entry.path().to_path_buf(),
                    start_line: r.start_line,
                    ident: r.ident,
                    path: r.path,
                    feature: r.feature,
                });
            }

            if !file_edits.is_empty() {
                edits_by_file.insert(entry.path().to_path_buf(), file_edits);
            }
        }
    }

    Ok(PruneFindings { reports, edits_by_file })
}

struct LocalReport {
    start_line: u32,
    ident: String,
    path: String,
    feature: String,
}

fn process_file(text: &str, manifest: &Manifest) -> (Vec<LocalReport>, Vec<UseEdit>) {
    let file = match syn::parse_file(text) {
        Ok(f) => f,
        Err(_) => return (Vec::new(), Vec::new()),
    };

    let referenced_in_body = collect_body_refs(&file);
    let mut reports = Vec::new();
    let mut edits = Vec::new();

    for item in &file.items {
        let Item::Use(use_item) = item else { continue };

        let span = use_item.span();
        let start_line = span.start().line.saturating_sub(1) as u32;
        let end_line = span.end().line.saturating_sub(1) as u32;
        let indent = leading_indent(text, start_line as usize);

        match classify_use(&use_item.tree, manifest) {
            UseShape::Single { leaf } => {
                if is_alive(&leaf.local_name, &referenced_in_body) {
                } else {
                    reports.push(LocalReport {
                        start_line,
                        ident: leaf.local_name.clone(),
                        path: leaf.full_path.clone(),
                        feature: leaf.feature.clone(),
                    });
                    edits.push(UseEdit::DropItem { start_line, end_line });
                }
            }
            UseShape::Group { prefix, members } => {
                let mut live_text_parts: Vec<String> = Vec::new();
                let mut any_dead = false;
                let mut any_live_engage = false;
                let mut any_non_engage = false;

                for m in &members {
                    match &m.kind {
                        MemberKind::Engage { leaf } => {
                            if is_alive(&leaf.local_name, &referenced_in_body) {
                                live_text_parts.push(m.original_text.clone());
                                any_live_engage = true;
                            } else {
                                any_dead = true;
                                reports.push(LocalReport {
                                    start_line,
                                    ident: leaf.local_name.clone(),
                                    path: leaf.full_path.clone(),
                                    feature: leaf.feature.clone(),
                                });
                            }
                        }
                        MemberKind::NonEngage => {
                            live_text_parts.push(m.original_text.clone());
                            any_non_engage = true;
                        }
                    }
                }

                if !any_dead {
                    continue;
                }

                if !any_live_engage && !any_non_engage {
                    edits.push(UseEdit::DropItem { start_line, end_line });
                } else {
                    let prefix_str = prefix.join("::");
                    let body = if live_text_parts.len() == 1 && !prefix_str.is_empty() {
                        format!("{}::{}", prefix_str, live_text_parts[0])
                    } else if prefix_str.is_empty() {
                        format!("{{{}}}", live_text_parts.join(", "))
                    } else {
                        format!("{}::{{{}}}", prefix_str, live_text_parts.join(", "))
                    };
                    edits.push(UseEdit::Replace {
                        start_line,
                        end_line,
                        new_text: format!("{}use {};", indent, body),
                    });
                }
            }
            UseShape::Other => {}
        }
    }

    (reports, edits)
}

struct EngageLeaf {
    full_path: String,
    local_name: String,
    feature: String,
}

enum MemberKind {
    Engage { leaf: EngageLeaf },
    NonEngage,
}

struct GroupMember {
    original_text: String,
    kind: MemberKind,
}

enum UseShape {
    Single { leaf: EngageLeaf },
    Group {
        prefix: Vec<String>,
        members: Vec<GroupMember>,
    },
    Other,
}

fn classify_use(tree: &UseTree, manifest: &Manifest) -> UseShape {
    let mut prefix: Vec<String> = Vec::new();
    let mut cur = tree;

    loop {
        match cur {
            UseTree::Path(p) => {
                prefix.push(strip_raw(&p.ident.to_string()));
                cur = &p.tree;
            }
            UseTree::Name(n) => {
                let name = strip_raw(&n.ident.to_string());
                let mut full = prefix.clone();
                full.push(name.clone());
                let path = full.join("::");
                let Some((full_path, feature)) = manifest.resolve_path(&path) else {
                    return UseShape::Other;
                };
                return UseShape::Single {
                    leaf: EngageLeaf {
                        full_path: full_path.to_string(),
                        local_name: name,
                        feature: feature.to_string(),
                    },
                };
            }
            UseTree::Rename(r) => {
                let name = strip_raw(&r.ident.to_string());
                let local = strip_raw(&r.rename.to_string());
                let mut full = prefix.clone();
                full.push(name.clone());
                let path = full.join("::");
                let Some((full_path, feature)) = manifest.resolve_path(&path) else {
                    return UseShape::Other;
                };
                return UseShape::Single {
                    leaf: EngageLeaf {
                        full_path: full_path.to_string(),
                        local_name: local,
                        feature: feature.to_string(),
                    },
                };
            }
            UseTree::Group(g) => {
                let mut members = Vec::new();
                for item in &g.items {
                    match item {
                        UseTree::Name(n) => {
                            let name = strip_raw(&n.ident.to_string());
                            let mut full = prefix.clone();
                            full.push(name.clone());
                            let path = full.join("::");
                            let kind = if let Some((full_path, feature)) = manifest.resolve_path(&path) {
                                MemberKind::Engage {
                                    leaf: EngageLeaf {
                                        full_path: full_path.to_string(),
                                        local_name: name.clone(),
                                        feature: feature.to_string(),
                                    },
                                }
                            } else {
                                MemberKind::NonEngage
                            };
                            members.push(GroupMember {
                                original_text: name,
                                kind,
                            });
                        }
                        UseTree::Rename(r) => {
                            let name = strip_raw(&r.ident.to_string());
                            let local = strip_raw(&r.rename.to_string());
                            let mut full = prefix.clone();
                            full.push(name.clone());
                            let path = full.join("::");
                            let kind = if let Some((full_path, feature)) = manifest.resolve_path(&path) {
                                MemberKind::Engage {
                                    leaf: EngageLeaf {
                                        full_path: full_path.to_string(),
                                        local_name: local.clone(),
                                        feature: feature.to_string(),
                                    },
                                }
                            } else {
                                MemberKind::NonEngage
                            };
                            members.push(GroupMember {
                                original_text: format!("{} as {}", name, local),
                                kind,
                            });
                        }
                        _ => return UseShape::Other,
                    }
                }
                return UseShape::Group { prefix, members };
            }
            UseTree::Glob(_) => return UseShape::Other,
        }
    }
}

fn is_alive(local_name: &str, referenced_in_body: &BTreeSet<String>) -> bool {
    if referenced_in_body.contains(local_name) {
        return true;
    }

    let bytes = local_name.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'I' && bytes[1].is_ascii_uppercase() {
        return true;
    }

    false
}

fn leading_indent(text: &str, start_line: usize) -> String {
    let line = text.lines().nth(start_line).unwrap_or("");
    line.chars().take_while(|c| c.is_whitespace() && *c != '\n').collect()
}

fn strip_raw(s: &str) -> String {
    s.strip_prefix("r#").unwrap_or(s).to_string()
}

fn collect_body_refs(file: &syn::File) -> BTreeSet<String> {
    let mut visitor = BodyRefVisitor { out: BTreeSet::new() };

    for item in &file.items {
        if matches!(item, Item::Use(_)) {
            continue;
        }

        visitor.visit_item(item);
    }

    visitor.out
}

struct BodyRefVisitor {
    out: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for BodyRefVisitor {
    fn visit_path(&mut self, path: &'ast syn::Path) {
        if let Some(first) = path.segments.first() {
            self.out.insert(strip_raw(&first.ident.to_string()));
        }

        syn::visit::visit_path(self, path);
    }

    fn visit_ident(&mut self, ident: &'ast proc_macro2::Ident) {
        self.out.insert(strip_raw(&ident.to_string()));
    }

    // syn keeps a macro's contents as raw tokens and never parses them, so a type
    // used only inside something like `vec![Fade::black_out(...)]` is invisible to
    // the normal walk. Pull every identifier out of the macro tokens so we don't
    // call a still-used import dead and delete it.
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        collect_idents_from_tokens(mac.tokens.clone(), &mut self.out);
        syn::visit::visit_macro(self, mac);
    }
}

fn collect_idents_from_tokens(tokens: proc_macro2::TokenStream, out: &mut BTreeSet<String>) {
    for tt in tokens {
        match tt {
            proc_macro2::TokenTree::Ident(id) => {
                out.insert(strip_raw(&id.to_string()));
            }
            proc_macro2::TokenTree::Group(g) => collect_idents_from_tokens(g.stream(), out),
            _ => {}
        }
    }
}

pub fn apply_use_edits(file: &Path, edits: &[UseEdit]) -> Result<()> {
    let raw = std::fs::read_to_string(file)
        .with_context(|| format!("reading {}", file.display()))?;
    let trailing_newline = raw.ends_with('\n');
    let mut lines: Vec<String> = raw.split('\n').map(String::from).collect();

    let mut sorted: Vec<&UseEdit> = edits.iter().collect();
    sorted.sort_by_key(|e| std::cmp::Reverse(match e {
        UseEdit::DropItem { start_line, .. } => *start_line,
        UseEdit::Replace { start_line, .. } => *start_line,
    }));

    for e in sorted {
        match e {
            UseEdit::DropItem { start_line, end_line } => {
                let s = *start_line as usize;
                let e = *end_line as usize;
                if e < lines.len() && s <= e {
                    lines.drain(s..=e);
                }
            }
            UseEdit::Replace { start_line, end_line, new_text } => {
                let s = *start_line as usize;
                let e = *end_line as usize;
                if e < lines.len() && s <= e {
                    lines.drain(s..=e);
                    for (i, nl) in new_text.split('\n').enumerate() {
                        lines.insert(s + i, nl.to_string());
                    }
                }
            }
        }
    }

    let mut joined = lines.join("\n");
    if trailing_newline && !joined.ends_with('\n') {
        joined.push('\n');
    }

    std::fs::write(file, joined).with_context(|| format!("writing {}", file.display()))?;

    Ok(())
}
