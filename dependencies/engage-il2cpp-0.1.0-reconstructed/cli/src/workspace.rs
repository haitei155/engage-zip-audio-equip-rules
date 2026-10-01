use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use cargo_metadata::MetadataCommand;

#[derive(Clone)]
pub struct Layout {
    pub features_manifest: PathBuf,
    pub scan_roots: Vec<PathBuf>,
    pub bindings_root: PathBuf,
    pub check_manifest: PathBuf,
}

pub fn detect(start: &std::path::Path) -> Result<Layout> {
    try_detect(start)?.ok_or_else(|| anyhow!("no workspace member depends on `engage-il2cpp`. Run from within a project that uses it"))
}

pub fn try_detect(start: &std::path::Path) -> Result<Option<Layout>> {
    Ok(detect_all(start)?.into_iter().next())
}

pub fn detect_all(start: &std::path::Path) -> Result<Vec<Layout>> {
    let metadata = MetadataCommand::new().current_dir(start).exec().context("running cargo metadata")?;

    let bindings = match metadata.packages.iter().find(|p| p.name.as_str() == "engage-il2cpp") {
        Some(p) => p,
        None => return Ok(Vec::new()),
    };

    let bindings_root: PathBuf = bindings.manifest_path.parent().unwrap().into();
    let project_root = metadata.workspace_root.as_std_path();

    let dependents: Vec<_> = metadata
        .packages
        .iter()
        .filter(|p| p.manifest_path.as_std_path().starts_with(project_root))
        .filter(|p| p.manifest_path.as_std_path() != bindings.manifest_path.as_std_path())
        .filter(|p| p.dependencies.iter().any(|d| d.name.as_str() == "engage-il2cpp"))
        .collect();

    let mut layouts: Vec<Layout> = dependents
        .iter()
        .map(|p| {
            let dir: PathBuf = p.manifest_path.parent().unwrap().into();
            let manifest_path: PathBuf = p.manifest_path.clone().into();

            Layout {
                features_manifest: manifest_path.clone(),
                scan_roots: [dir.join("src")].into_iter().filter(|p| p.exists()).collect(),
                bindings_root: bindings_root.clone(),
                check_manifest: manifest_path,
            }
        })
        .collect();

    if let Some(ws_layout) = workspace_dependency_layout(&metadata, &bindings_root) {
        layouts.insert(0, ws_layout);
    }

    Ok(layouts)
}

fn workspace_dependency_layout(metadata: &cargo_metadata::Metadata, bindings_root: &Path) -> Option<Layout> {
    let ws_root = metadata.workspace_root.as_std_path();
    let ws_toml = ws_root.join("Cargo.toml");

    let raw = std::fs::read_to_string(&ws_toml).ok()?;
    let doc = raw.parse::<toml_edit::DocumentMut>().ok()?;

    let has_ws_dep = doc
        .get("workspace")
        .and_then(|t| t.get("dependencies"))
        .and_then(|t| t.get("engage-il2cpp"))
        .is_some();

    if !has_ws_dep {
        return None;
    }

    Some(Layout {
        features_manifest: ws_toml.clone(),
        scan_roots: Vec::new(),
        bindings_root: bindings_root.to_path_buf(),
        check_manifest: ws_toml,
    })
}
