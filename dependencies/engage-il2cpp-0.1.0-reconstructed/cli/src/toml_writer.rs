use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use toml_edit::{Array, DocumentMut, Item, Value};

pub fn read_features(manifest_path: &Path) -> Result<BTreeSet<String>> {
    let raw = std::fs::read_to_string(manifest_path).with_context(|| format!("reading {}", manifest_path.display()))?;
    let doc: DocumentMut = raw.parse().with_context(|| format!("parsing {}", manifest_path.display()))?;
    let dep = locate_dep(&doc).ok_or_else(|| anyhow!("no `engage-il2cpp` dependency declared in {}", manifest_path.display()))?;

    let mut out = BTreeSet::new();

    if let Some(arr) = dep.get("features").and_then(Item::as_array) {
        for v in arr.iter() {
            if let Some(s) = v.as_str() {
                out.insert(s.to_string());
            }
        }
    }

    Ok(out)
}

pub fn write_features(manifest_path: &Path, features: &BTreeSet<String>) -> Result<()> {
    let raw = std::fs::read_to_string(manifest_path).with_context(|| format!("reading {}", manifest_path.display()))?;
    let mut doc: DocumentMut = raw.parse().with_context(|| format!("parsing {}", manifest_path.display()))?;

    let dep = locate_dep_mut(&mut doc).ok_or_else(|| anyhow!("no `engage-il2cpp` dependency declared in {}", manifest_path.display()))?;

    let mut arr = Array::new();

    for f in features {
        arr.push(f.as_str());
    }

    arr.fmt();

    let mut multiline = arr.clone();
    set_multiline(&mut multiline);

    if dep.is_inline_table() || dep.is_table() {
        if let Some(table) = dep.as_table_like_mut() {
            table.insert("features", Item::Value(Value::Array(multiline)));
        }
    } else {
        return Err(anyhow!("engage-il2cpp dep is a string, not a table, cannot set features"));
    }

    std::fs::write(manifest_path, doc.to_string()).with_context(|| format!("writing {}", manifest_path.display()))?;

    Ok(())
}

fn locate_dep(doc: &DocumentMut) -> Option<&Item> {
    if let Some(deps) = doc.get("dependencies") {
        if let Some(d) = deps.get("engage-il2cpp") {
            return Some(d);
        }
    }

    if let Some(workspace) = doc.get("workspace") {
        if let Some(deps) = workspace.get("dependencies") {
            if let Some(d) = deps.get("engage-il2cpp") {
                return Some(d);
            }
        }
    }

    None
}

fn locate_dep_mut(doc: &mut DocumentMut) -> Option<&mut Item> {
    if doc.get("dependencies").and_then(|t| t.get("engage-il2cpp")).is_some() {
        return doc.get_mut("dependencies").and_then(|t| t.get_mut("engage-il2cpp"));
    }

    if doc
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(|d| d.get("engage-il2cpp"))
        .is_some()
    {
        return doc
            .get_mut("workspace")
            .and_then(|w| w.get_mut("dependencies"))
            .and_then(|d| d.get_mut("engage-il2cpp"));
    }

    None
}

fn set_multiline(arr: &mut Array) {
    for v in arr.iter_mut() {
        let decor = v.decor_mut();
        decor.set_prefix("\n    ");
        decor.set_suffix("");
    }

    arr.set_trailing("\n");
    arr.set_trailing_comma(true);
}
