use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use anyhow::Result;
use serde::Deserialize;

use crate::manifest::Manifest;

pub fn collect_unresolved_paths(manifest_path: &Path, manifest: &Manifest) -> Result<BTreeSet<String>> {
    let output = Command::new("cargo")
        .arg("check")
        .arg("--message-format=json")
        .arg("--manifest-path")
        .arg(manifest_path)
        .output()?;

    let crate_ident = manifest.crate_ident();
    let mut found = BTreeSet::new();

    for line in output.stdout.split(|b| *b == b'\n') {
        if line.is_empty() {
            continue;
        }

        let msg: CargoMessage = match serde_json::from_slice(line) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let CargoMessage::CompilerMessage { message } = msg else { continue };

        if message.code.as_ref().map(|c| c.code.as_str()) != Some("E0432") && message.code.as_ref().map(|c| c.code.as_str()) != Some("E0433") {
            continue;
        }

        for cand in extract_paths(&message.message, &crate_ident) {
            if manifest.paths.contains_key(&cand) {
                found.insert(cand);
            }
        }
    }

    Ok(found)
}

fn extract_paths(msg: &str, crate_ident: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = msg.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if !msg[i..].starts_with(crate_ident) {
            i += 1;
            continue;
        }

        let start = i;
        let mut j = start + crate_ident.len();

        while j < bytes.len() {
            let rest = &msg[j..];

            if rest.starts_with("::") {
                j += 2;
                continue;
            }

            let c = bytes[j];

            if c.is_ascii_alphanumeric() || c == b'_' {
                j += 1;
            } else {
                break;
            }
        }

        let mut end = j;

        while end > start && msg[start..end].ends_with("::") {
            end -= 2;
        }

        if end > start + crate_ident.len() {
            out.push(msg[start..end].to_string());
        }

        i = j.max(start + 1);
    }

    out
}

#[derive(Debug, Deserialize)]
#[serde(tag = "reason", rename_all = "kebab-case")]
enum CargoMessage {
    CompilerMessage {
        message: Diagnostic,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct Diagnostic {
    message: String,
    code: Option<DiagnosticCode>,
}

#[derive(Debug, Deserialize)]
struct DiagnosticCode {
    code: String,
}
