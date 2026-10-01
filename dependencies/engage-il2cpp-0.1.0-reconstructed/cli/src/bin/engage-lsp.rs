use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result as JsonRpcResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use engage::commands;
use engage::cover;
use engage::manifest::Manifest;
use engage::quickfix;
use engage::toml_writer;
use engage::workspace;

struct Backend {
    client: Client,
    state: Arc<Mutex<BackendState>>,
}

#[derive(Default)]
struct BackendState {
    root: PathBuf,
    crates: Vec<CrateState>,
    last_uris: Vec<Url>,
}

struct CrateState {
    layout: workspace::Layout,
    manifest: Manifest,
    listed: BTreeSet<String>,
    required: BTreeSet<String>,
}

impl Backend {
    async fn refresh(&self) {
        let state_arc = Arc::clone(&self.state);
        let prev_uris = state_arc.lock().await.last_uris.clone();
        let root = state_arc.lock().await.root.clone();

        let scans = match tokio::task::spawn_blocking(move || {
            std::env::set_current_dir(&root)?;
            commands::scan_all_layouts(false)
        })
        .await
        {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                self.client.log_message(MessageType::ERROR, format!("scan failed: {e}")).await;
                return;
            },
            Err(e) => {
                self.client.log_message(MessageType::ERROR, format!("scan task crashed: {e}")).await;
                return;
            },
        };

        let mut new_crates: Vec<CrateState> = Vec::with_capacity(scans.len());
        let mut new_uris: Vec<Url> = Vec::with_capacity(scans.len());

        for scanned in scans {
            let listed = match toml_writer::read_features(&scanned.layout.features_manifest) {
                Ok(l) => l,
                Err(e) => {
                    self.client.log_message(MessageType::ERROR, format!("read_features failed: {e}")).await;
                    continue;
                },
            };

            let required = cover::minimum_cover(&scanned.referenced_features, &scanned.manifest);
            let (missing, _stale) = commands::diff_features(&listed, &required, &scanned.manifest);

            if let Some(uri) = Url::from_file_path(&scanned.layout.features_manifest).ok() {
                self.publish_diagnostics(uri.clone(), &scanned.layout.features_manifest, &missing).await;
                new_uris.push(uri);
            }

            new_crates.push(CrateState {
                layout: scanned.layout,
                manifest: scanned.manifest,
                listed,
                required,
            });
        }

        for uri in prev_uris {
            if !new_uris.contains(&uri) {
                self.client.publish_diagnostics(uri, Vec::new(), None).await;
            }
        }

        let mut state = state_arc.lock().await;
        state.crates = new_crates;
        state.last_uris = new_uris;
    }

    async fn refresh_quickfix_for(&self, uri: Url) {
        let Some(file_path) = uri.to_file_path().ok() else { return };

        if file_path.extension().and_then(|s| s.to_str()) != Some("rs") {
            return;
        }

        let Ok(text) = std::fs::read_to_string(&file_path) else { return };

        let state = self.state.lock().await;

        if state.crates.is_empty() {
            return;
        }

        let mut diagnostics = Vec::new();

        for crate_state in &state.crates {
            for found in quickfix::find_unresolved(&text, &crate_state.manifest) {
                let already_listed = crate_state.listed.contains(&found.feature);
                let title_prefix = if already_listed { "Import" } else { "Import + enable" };

                let data = serde_json::json!({
                    "kind": "engage.quickfix",
                    "feature": found.feature,
                    "import_path": found.import_path,
                    "ident": found.ident,
                    "use_insert_line": found.use_insert_line,
                    "already_listed": already_listed,
                    "title_prefix": title_prefix,
                });

                diagnostics.push(Diagnostic {
                    range: Range {
                        start: Position { line: found.start_line, character: found.start_col },
                        end: Position { line: found.end_line, character: found.end_col },
                    },
                    severity: Some(DiagnosticSeverity::HINT),
                    source: Some("engage".into()),
                    code: Some(NumberOrString::String("unresolved-type".into())),
                    message: format!("`{}` is an engage-il2cpp type. Import as `{}`.", found.ident, found.import_path),
                    data: Some(data),
                    ..Default::default()
                });
            }
        }

        self.client.publish_diagnostics(uri, diagnostics, None).await;
    }

    async fn publish_diagnostics(&self, uri: Url, cargo_toml: &Path, missing: &BTreeSet<String>) {
        let mut diagnostics = Vec::new();
        let range = engage_dep_range(Some(cargo_toml)).unwrap_or_else(|| Range {
            start: Position { line: 0, character: 0 },
            end: Position { line: 0, character: 0 },
        });

        if !missing.is_empty() {
            let list: Vec<&String> = missing.iter().collect();
            let message = format!(
                "engage-il2cpp: missing {} required feature{}: {}",
                missing.len(),
                if missing.len() == 1 { "" } else { "s" },
                list.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "),
            );
            diagnostics.push(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::WARNING),
                source: Some("engage".into()),
                code: Some(NumberOrString::String("missing-features".into())),
                message,
                ..Default::default()
            });
        }

        self.client.publish_diagnostics(uri, diagnostics, None).await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> JsonRpcResult<InitializeResult> {
        let root = workspace_root(&params).unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        {
            let mut state = self.state.lock().await;
            state.root = root;
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(TextDocumentSyncOptions {
                    open_close: Some(true),
                    save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                    change: Some(TextDocumentSyncKind::NONE),
                    ..Default::default()
                })),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "engage-lsp".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.refresh().await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        self.refresh().await;
        self.refresh_quickfix_for(params.text_document.uri).await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.refresh_quickfix_for(params.text_document.uri).await;
    }

    async fn did_change_watched_files(&self, _: DidChangeWatchedFilesParams) {
        self.refresh().await;
    }

    async fn hover(&self, params: HoverParams) -> JsonRpcResult<Option<Hover>> {
        let state = self.state.lock().await;

        let uri = params.text_document_position_params.text_document.uri;
        let Some(file_path) = uri.to_file_path().ok() else { return Ok(None) };
        let Some(text) = std::fs::read_to_string(&file_path).ok() else { return Ok(None) };
        let position = params.text_document_position_params.position;

        for crate_state in &state.crates {
            let Some(path_at_cursor) = path_at_position(&text, position, &crate_state.manifest.crate_ident()) else {
                continue;
            };

            let Some(feature) = crate_state.manifest.paths.get(&path_at_cursor) else {
                continue;
            };

            let mut content = format!("**`{path_at_cursor}`**\n\nfeature: `{feature}`");

            if crate_state.required.contains(feature) && !crate_state.listed.iter().any(|l| crate_state.manifest.closure(l).contains(feature)) {
                content.push_str("\n\n_missing from Cargo.toml_");
            }

            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: content,
                }),
                range: None,
            }));
        }

        Ok(None)
    }

    async fn code_action(&self, params: CodeActionParams) -> JsonRpcResult<Option<CodeActionResponse>> {
        let mut actions = Vec::new();
        let uri = params.text_document.uri.clone();

        for diag in &params.context.diagnostics {
            if diag.source.as_deref() != Some("engage") {
                continue;
            }

            let code = match &diag.code {
                Some(NumberOrString::String(s)) => s.as_str(),
                _ => continue,
            };

            match code {
                "missing-features" => {
                    actions.push(CodeActionOrCommand::Command(Command {
                        title: "engage: Apply missing features".into(),
                        command: "engage.featuresApply".into(),
                        arguments: None,
                    }));
                },
                "unresolved-type" => {
                    if let Some(action) = build_unresolved_type_action(&uri, diag) {
                        actions.push(action);
                    }
                },
                _ => {},
            }
        }

        Ok(Some(actions))
    }

    async fn shutdown(&self) -> JsonRpcResult<()> {
        Ok(())
    }
}

fn build_unresolved_type_action(uri: &Url, diag: &Diagnostic) -> Option<CodeActionOrCommand> {
    let data = diag.data.as_ref()?;
    let feature = data.get("feature")?.as_str()?;
    let import_path = data.get("import_path")?.as_str()?;
    let ident = data.get("ident")?.as_str()?;
    let use_insert_line = data.get("use_insert_line")?.as_u64()? as u32;
    let already_listed = data.get("already_listed").and_then(|v| v.as_bool()).unwrap_or(false);
    let title_prefix = data.get("title_prefix")?.as_str()?;

    let use_text = format!("use {import_path};\n");

    let mut changes = std::collections::HashMap::new();
    changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range {
                start: Position { line: use_insert_line, character: 0 },
                end: Position { line: use_insert_line, character: 0 },
            },
            new_text: use_text,
        }],
    );

    let edit = WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    };

    let title = if already_listed {
        format!("engage: {title_prefix} `{ident}`")
    } else {
        format!("engage: {title_prefix} `{ident}` (feature `{feature}`)")
    };

    let command = if already_listed {
        None
    } else {
        Some(Command {
            title: "Add feature".into(),
            command: "engage.addFeature".into(),
            arguments: Some(vec![serde_json::Value::String(feature.to_string())]),
        })
    };

    Some(CodeActionOrCommand::CodeAction(CodeAction {
        title,
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diag.clone()]),
        edit: Some(edit),
        command,
        is_preferred: Some(true),
        ..Default::default()
    }))
}

fn workspace_root(params: &InitializeParams) -> Option<PathBuf> {
    if let Some(folders) = &params.workspace_folders {
        if let Some(folder) = folders.first() {
            if let Ok(p) = folder.uri.to_file_path() {
                return Some(p);
            }
        }
    }

    #[allow(deprecated)]
    if let Some(uri) = &params.root_uri {
        if let Ok(p) = uri.to_file_path() {
            return Some(p);
        }
    }

    None
}

fn engage_dep_range(cargo_toml: Option<&Path>) -> Option<Range> {
    let path = cargo_toml?;
    let raw = std::fs::read_to_string(path).ok()?;

    for (i, line) in raw.lines().enumerate() {
        if line.contains("engage-il2cpp") {
            return Some(Range {
                start: Position { line: i as u32, character: 0 },
                end: Position { line: i as u32, character: line.len() as u32 },
            });
        }
    }

    None
}

fn path_at_position(text: &str, position: Position, crate_ident: &str) -> Option<String> {
    let line = text.lines().nth(position.line as usize)?;
    let col = position.character as usize;

    if col > line.len() {
        return None;
    }

    let bytes = line.as_bytes();
    let mut start = col;

    while start > 0 {
        let c = bytes[start - 1];

        if c.is_ascii_alphanumeric() || c == b'_' || c == b':' || c == b'#' {
            start -= 1;
        } else {
            break;
        }
    }

    let mut end = col;

    while end < bytes.len() {
        let c = bytes[end];

        if c.is_ascii_alphanumeric() || c == b'_' || c == b':' || c == b'#' {
            end += 1;
        } else {
            break;
        }
    }

    if end <= start {
        return None;
    }

    let raw = &line[start..end];
    let normalized = raw.replace("r#", "");

    if !normalized.starts_with(crate_ident) {
        return None;
    }

    Some(normalized)
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        state: Arc::new(Mutex::new(BackendState::default())),
    });

    Server::new(stdin, stdout, socket).serve(service).await;
}
