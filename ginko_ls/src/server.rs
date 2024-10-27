use ginko::dts::analysis::project::Project;
use ginko::dts::ast::AstNode;
use ginko::dts::syntax::SyntaxKind;
use ginko::dts::{ast, ItemAtCursor, Severity, SeverityMap, TextRange, TextSize, WalkEvent};
use itertools::Itertools;
use line_index::{LineCol, LineIndex};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::mem;
use std::path::{Path, PathBuf};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use url::Url;

pub(crate) struct Backend {
    client: Client,
    project: RwLock<Project>,
    line_index_cache: RwLock<HashMap<PathBuf, LineIndex>>,
    severities: SeverityMap,
}

impl Backend {
    pub fn new(client: Client) -> Backend {
        Backend {
            client,
            project: RwLock::new(Project::default()),
            severities: SeverityMap::default(),
            line_index_cache: RwLock::new(HashMap::new()),
        }
    }
}

#[derive(Deserialize, Serialize, Default, Debug)]
struct ProjectConfig {
    pub includes: Vec<String>,
}

impl ProjectConfig {
    pub fn from_value(value: Value) -> Self {
        serde_json::from_value(value).unwrap_or_default()
    }
}

fn lsp_severity_from_severity(severity_level: Severity) -> DiagnosticSeverity {
    match severity_level {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Hint => DiagnosticSeverity::HINT,
    }
}

pub trait ToPos {
    fn to_pos(self) -> Position;
}

impl ToPos for LineCol {
    fn to_pos(self) -> Position {
        Position::new(self.line, self.col)
    }
}

impl Backend {
    fn pos_to_text_size(&self, position: Position, line_index: &LineIndex) -> Option<TextSize> {
        line_index.offset(LineCol {
            line: position.line,
            col: position.character,
        })
    }

    fn range_to_text_range(&self, range: Range, line_index: &LineIndex) -> Option<TextRange> {
        let start = self.pos_to_text_size(range.start, line_index)?;
        let end = self.pos_to_text_size(range.end, line_index)?;
        Some(TextRange::new(start, end))
    }

    fn text_range_to_range(&self, range: TextRange, line_index: &LineIndex) -> Range {
        let start = line_index.line_col(range.start()).to_pos();
        let end = line_index.line_col(range.end()).to_pos();
        Range::new(start, end)
    }

    fn lsp_diag_from_diag(
        &self,
        diagnostic: &ginko::dts::Diagnostic,
        line_index: &LineIndex,
    ) -> Diagnostic {
        Diagnostic {
            range: self.text_range_to_range(diagnostic.range, line_index),
            message: diagnostic.message.clone(),
            code: Some(NumberOrString::String(diagnostic.code.as_ref().to_string())),
            severity: Some(lsp_severity_from_severity(
                diagnostic.severity(&self.severities),
            )),
            source: Some("ginko_ls".to_string()),
            ..Default::default()
        }
    }

    async fn url_to_file_path(&self, url: Url) -> Option<PathBuf> {
        match url.to_file_path() {
            Ok(path) => Some(path),
            Err(_) => {
                self.client
                    .show_message(
                        MessageType::ERROR,
                        format!("Url {url} is not a valid file path"),
                    )
                    .await;
                None
            }
        }
    }

    async fn publish_diagnostics(&self) {
        let file_paths = self
            .project
            .read()
            .files()
            .map(|file| file.to_owned())
            .collect_vec();
        for file in file_paths {
            let diagnostics = {
                let cache = self.line_index_cache.read();
                let Some(line_index) = cache.get(&file) else {
                    debug_assert!(false, "Line index should be present");
                    continue;
                };
                self.project
                    .read()
                    .get_file(&file)
                    .map(|file| {
                        file.read()
                            .diagnostics()
                            .map(|diag| self.lsp_diag_from_diag(diag, &line_index))
                            .collect_vec()
                    })
                    .unwrap_or_default()
            };
            self.client
                .publish_diagnostics(Url::from_file_path(&file).unwrap(), diagnostics, None)
                .await
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let config = ProjectConfig::from_value(params.initialization_options.unwrap_or_default());
        self.project
            .write()
            .set_include_paths(config.includes.into_iter().map(PathBuf::from));

        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                definition_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {}

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let Some(file_path) = self.url_to_file_path(params.text_document.uri).await else {
            return;
        };
        let text = params.text_document.text;
        {
            let mut cache = self.line_index_cache.write();
            cache.insert(file_path.clone(), LineIndex::new(&text));
        }
        self.project.write().add_file(file_path.clone(), &text);
        self.publish_diagnostics().await
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let Some(file_path) = self.url_to_file_path(params.text_document.uri).await else {
            return;
        };
        let text = params.content_changes.into_iter().next().unwrap().text;
        {
            let mut cache = self.line_index_cache.write();
            cache.insert(file_path.clone(), LineIndex::new(&text));
        }
        self.project.write().add_file(file_path.clone(), &text);
        self.publish_diagnostics().await
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {}

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let Some(file_path) = self.url_to_file_path(params.text_document.uri).await else {
            return;
        };
        self.project.write().remove_file(&file_path);
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let Some(file_path) = self
            .url_to_file_path(params.text_document_position_params.text_document.uri)
            .await
        else {
            return Ok(None);
        };
        let project = self.project.read();
        let Some(file) = project.get_file(&file_path) else {
            return Ok(None);
        };
        let cache = self.line_index_cache.read();
        let Some(line_index) = cache.get(&file_path) else {
            return Ok(None);
        };
        let Some(pos) =
            self.pos_to_text_size(params.text_document_position_params.position, line_index)
        else {
            return Ok(None);
        };

        let file_inner = file.read();
        let Some(item) = file_inner.item_at_cursor(pos) else {
            return Ok(None);
        };
        match item {
            ItemAtCursor::Reference(reference) => {
                match project.get_node_position(&file_path, reference) {
                    Some((span, path)) => Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                        Url::from_file_path(path).unwrap(),
                        self.text_range_to_range(span, line_index),
                    )))),
                    None => Ok(None),
                }
            }
            ItemAtCursor::Include(include) => {
                match Url::from_file_path(Path::new(include.file_name.item())) {
                    Ok(url) => Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                        url,
                        Range::default(),
                    )))),
                    _ => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let Some(file_path) = self
            .url_to_file_path(params.text_document_position_params.text_document.uri)
            .await
        else {
            return Ok(None);
        };
        let project = self.project.read();
        let Some(file) = project.get_file(&file_path) else {
            return Ok(None);
        };
        let cache = self.line_index_cache.read();
        let Some(line_index) = cache.get(&file_path) else {
            return Ok(None);
        };
        let Some(pos) =
            self.pos_to_text_size(params.text_document_position_params.position, line_index)
        else {
            return Ok(None);
        };

        let file_inner = file.read();
        let Some(item) = file_inner.item_at_cursor(pos) else {
            return Ok(None);
        };
        let str = match item {
            ItemAtCursor::Reference(reference) => {
                match project.document_reference(&file_path, reference) {
                    Some(str) => str,
                    None => return Ok(None),
                }
            }
            ItemAtCursor::Label(name) => name.item().clone(),
            ItemAtCursor::Include(include) => include.file_name.item().clone(),
            _ => return Ok(None),
        };

        Ok(Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(str.to_string())),
            range: None,
        }))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let Some(file_path) = self.url_to_file_path(params.text_document.uri).await else {
            return Ok(None);
        };
        let project = self.project.read();
        let Some(file) = project.get_file(&file_path) else {
            return Ok(None);
        };
        let read_guard = file.read();
        let ast = read_guard.ast();
        let symbols = self.generate_nested_symbol_response(&file_path, &ast);
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }
}

impl Backend {
    fn generate_nested_symbol_response(
        &self,
        path: &PathBuf,
        ast: &ast::File,
    ) -> Vec<DocumentSymbol> {
        let cache = self.line_index_cache.read();
        let Some(line_index) = cache.get(path) else {
            return Vec::default();
        };
        let mut stack = Vec::new();
        let mut current = Vec::new();
        for item in ast.walk() {
            match item {
                WalkEvent::Enter(syntax_node) => match syntax_node.kind() {
                    SyntaxKind::NODE => {
                        let node = ast::Node::cast(syntax_node).unwrap();
                        #[allow(deprecated)]
                        let symbol = DocumentSymbol {
                            name: node.name().text(),
                            detail: None,
                            kind: SymbolKind::MODULE,
                            tags: None,
                            deprecated: None,
                            range: self.text_range_to_range(node.range(), line_index),
                            selection_range: self
                                .text_range_to_range(node.name().range(), line_index),
                            children: Some(vec![]),
                        };
                        stack.push((symbol, mem::take(&mut current)));
                    }
                    SyntaxKind::PROPERTY => {
                        let prop = ast::Property::cast(syntax_node).unwrap();
                        #[allow(deprecated)]
                        let symbol = DocumentSymbol {
                            name: prop.name().text(),
                            detail: None,
                            kind: SymbolKind::PROPERTY,
                            tags: None,
                            deprecated: None,
                            range: self.text_range_to_range(prop.range(), line_index),
                            selection_range: self
                                .text_range_to_range(prop.name().range(), line_index),
                            children: None,
                        };
                        current.push(symbol);
                    }
                    _ => {}
                },
                WalkEvent::Leave(node) => {
                    if node.kind() == SyntaxKind::NODE {
                        let (mut symbol, new_current) =
                            stack.pop().expect("Unbalanced push / pop from stack");
                        symbol.children = Some(mem::replace(&mut current, new_current));
                        current.push(symbol);
                    }
                }
            }
        }

        current
    }
}
