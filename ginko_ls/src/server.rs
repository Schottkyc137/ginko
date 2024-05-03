use ginko::dts::{
    AnyDirective, FileType, HasSpan, ItemAtCursor, Node, NodeItem, NodePayload, Primary, Project,
    Severity, SeverityMap, Span,
};
use itertools::Itertools;
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use url::Url;

pub(crate) struct Backend {
    client: Client,
    project: RwLock<Project>,
    severities: SeverityMap,
}

impl Backend {
    pub fn new(client: Client) -> Backend {
        Backend {
            client,
            project: RwLock::new(Project::default()),
            severities: SeverityMap::default(),
        }
    }
}

fn lsp_severity_from_severity(severity_level: Severity) -> DiagnosticSeverity {
    match severity_level {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Hint => DiagnosticSeverity::HINT,
    }
}

fn lsp_range_from_span(span: Span) -> Range {
    Range::new(lsp_pos_from_pos(span.start()), lsp_pos_from_pos(span.end()))
}

fn lsp_pos_from_pos(pos: ginko::dts::Position) -> Position {
    Position::new(pos.line(), pos.character())
}

impl Backend {
    fn lsp_diag_from_diag(&self, diagnostic: &ginko::dts::Diagnostic) -> Diagnostic {
        let span = diagnostic.span();
        Diagnostic {
            range: lsp_range_from_span(span),
            message: diagnostic.message.clone(),
            code: Some(NumberOrString::String(diagnostic.kind.as_ref().to_string())),
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
            let diagnostics = self
                .project
                .read()
                .get_diagnostics(&file)
                .map(|diag| self.lsp_diag_from_diag(diag))
                .collect_vec();
            self.client
                .publish_diagnostics(Url::from_file_path(&file).unwrap(), diagnostics, None)
                .await
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
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
        let file_type = FileType::from(file_path.as_path());
        if file_type == FileType::Unknown {
            self.client.show_message(MessageType::WARNING, format!("File {} cannot be associated to a device-tree source. Make sure it has the ending 'dts', 'dtsi' or 'dtso'", file_path.to_string_lossy())).await;
            return;
        }
        self.project.write().add_file_with_text(
            file_path.clone(),
            params.text_document.text,
            file_type,
        );
        self.publish_diagnostics().await
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let Some(file_path) = self.url_to_file_path(params.text_document.uri).await else {
            return;
        };
        let file_type = FileType::from(file_path.as_path());
        self.project.write().add_file_with_text(
            file_path.clone(),
            params.content_changes.into_iter().next().unwrap().text,
            file_type,
        );
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
        let pos = position_to_ginko_position(params.text_document_position_params.position);
        let project = self.project.read();
        let Some(item) = project.find_at_pos(&file_path, &pos) else {
            return Ok(None);
        };
        match item {
            ItemAtCursor::Reference(reference) => {
                match project.get_node_position(&file_path, reference) {
                    Some((span, path)) => Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                        Url::from_file_path(path).unwrap(),
                        ginko_span_to_range(span),
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
        let pos = position_to_ginko_position(params.text_document_position_params.position);
        let project = self.project.read();
        let Some(item) = project.find_at_pos(&file_path, &pos) else {
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
        let Some(root) = project.get_root(&file_path) else {
            return Ok(None);
        };
        #[allow(deprecated)]
        fn node_payload_to_symbol(payload: &NodePayload) -> Vec<DocumentSymbol> {
            let mut children: Vec<DocumentSymbol> = Vec::new();
            for el in &payload.items {
                let document_symbol = match el {
                    NodeItem::Property(prop) => DocumentSymbol {
                        name: prop.name.item().clone(),
                        detail: None,
                        kind: SymbolKind::PROPERTY,
                        tags: None,
                        deprecated: None,
                        range: ginko_span_to_range(prop.span()),
                        selection_range: ginko_span_to_range(prop.name.span()),
                        children: None,
                    },
                    NodeItem::Node(node) => node_to_symbol(node),
                    NodeItem::DeletedNode(start_tok, deleted_node) => DocumentSymbol {
                        name: format!("{}", deleted_node.item()),
                        detail: None,
                        kind: SymbolKind::MODULE,
                        tags: Some(vec![SymbolTag::DEPRECATED]),
                        deprecated: None,
                        range: ginko_span_to_range(Span::new(
                            start_tok.start(),
                            deleted_node.end(),
                        )),
                        selection_range: ginko_span_to_range(deleted_node.span()),
                        children: None,
                    },
                    NodeItem::DeletedProperty(start_tok, deleted_property) => DocumentSymbol {
                        name: deleted_property.item().to_string(),
                        detail: None,
                        kind: SymbolKind::PROPERTY,
                        tags: Some(vec![SymbolTag::DEPRECATED]),
                        deprecated: None,
                        range: ginko_span_to_range(Span::new(
                            start_tok.start(),
                            deleted_property.end(),
                        )),
                        selection_range: ginko_span_to_range(deleted_property.span()),
                        children: None,
                    },
                };
                children.push(document_symbol);
            }
            children
        }
        #[allow(deprecated)]
        fn node_to_symbol(node: &Node) -> DocumentSymbol {
            DocumentSymbol {
                name: node.name.name.clone(),
                detail: None,
                kind: SymbolKind::MODULE,
                tags: None,
                deprecated: None,
                range: ginko_span_to_range(node.span()),
                selection_range: ginko_span_to_range(node.name.span()),
                children: Some(node_payload_to_symbol(&node.payload)),
            }
        }
        #[allow(deprecated)]
        let nodes = root
            .elements
            .iter()
            .filter_map(|el| match el {
                Primary::Root(node) => Some(node_to_symbol(node)),
                Primary::ReferencedNode(ref_node) => Some(DocumentSymbol {
                    name: format!("{}", ref_node.reference),
                    detail: None,
                    kind: SymbolKind::MODULE,
                    tags: None,
                    deprecated: None,
                    range: ginko_span_to_range(ref_node.span()),
                    selection_range: ginko_span_to_range(ref_node.reference.span()),
                    children: Some(node_payload_to_symbol(&ref_node.payload)),
                }),
                Primary::Directive(AnyDirective::Include(include)) => Some(DocumentSymbol {
                    name: format!("include {}", include.file_name.item()),
                    detail: None,
                    kind: SymbolKind::FILE,
                    tags: None,
                    deprecated: None,
                    range: ginko_span_to_range(include.span()),
                    selection_range: ginko_span_to_range(include.file_name.span()),
                    children: None,
                }),
                Primary::DeletedNode(token, ref_node) => Some(DocumentSymbol {
                    name: format!("{}", ref_node),
                    detail: None,
                    kind: SymbolKind::MODULE,
                    tags: Some(vec![SymbolTag::DEPRECATED]),
                    deprecated: None,
                    range: ginko_span_to_range(Span::new(token.start(), ref_node.end())),
                    selection_range: ginko_span_to_range(ref_node.span()),
                    children: None,
                }),
                _ => None,
            })
            .collect_vec();
        Ok(Some(DocumentSymbolResponse::Nested(nodes)))
    }
}

fn ginko_span_to_range(span: Span) -> Range {
    Range::new(
        ginko_position_to_position(span.start()),
        ginko_position_to_position(span.end()),
    )
}

#[allow(unused)]
fn range_to_ginko_span(range: Range) -> Span {
    Span::new(
        position_to_ginko_position(range.start),
        position_to_ginko_position(range.end),
    )
}

fn position_to_ginko_position(position: Position) -> ginko::dts::Position {
    ginko::dts::Position::new(position.line, position.character)
}

fn ginko_position_to_position(position: ginko::dts::Position) -> Position {
    Position::new(position.line(), position.character())
}
