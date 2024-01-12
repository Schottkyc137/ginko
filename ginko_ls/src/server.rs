use ginko::dts::{
    AnyDirective, FileType, HasSpan, ItemAtCursor, Node, NodePayload, Primary, Project,
    SeverityLevel, Span,
};
use itertools::Itertools;
use parking_lot::RwLock;
use std::path::Path;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use url::Url;

pub(crate) struct Backend {
    client: Client,
    project: RwLock<Project>,
}

impl Backend {
    pub fn new(client: Client) -> Backend {
        Backend {
            client,
            project: RwLock::new(Project::default()),
        }
    }
}

fn lsp_severity_from_severity(severity_level: SeverityLevel) -> DiagnosticSeverity {
    match severity_level {
        SeverityLevel::Error => DiagnosticSeverity::ERROR,
        SeverityLevel::Warning => DiagnosticSeverity::WARNING,
        SeverityLevel::Hint => DiagnosticSeverity::HINT,
    }
}

fn lsp_diag_from_diag(diagnostic: &ginko::dts::Diagnostic) -> Diagnostic {
    let span = diagnostic.span();
    Diagnostic {
        range: lsp_range_from_span(span),
        message: format!("{}", diagnostic.kind()),
        severity: Some(lsp_severity_from_severity(diagnostic.default_severity())),
        ..Default::default()
    }
}

fn lsp_range_from_span(span: Span) -> Range {
    Range::new(lsp_pos_from_pos(span.start()), lsp_pos_from_pos(span.end()))
}

fn lsp_pos_from_pos(pos: ginko::dts::Position) -> Position {
    Position::new(pos.line(), pos.character())
}

fn guess_file_type(url: &Url) -> FileType {
    url.path()
        .split('.')
        .last()
        .map(FileType::from_file_ending)
        .unwrap_or(FileType::Unknown)
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
        let uri = params.text_document.uri;
        let file_type = guess_file_type(&uri);
        if file_type == FileType::Unknown {
            self.client.show_message(MessageType::WARNING, format!("File {uri} cannot be associated to a device-tree source. Make sure it has the ending 'dts', 'dtsi' or 'dtso'")).await;
            return;
        }
        self.project
            .write()
            .add_file(uri.clone(), params.text_document.text, file_type);
        let diagnostics = self
            .project
            .read()
            .get_diagnostics(&uri)
            .iter()
            .map(lsp_diag_from_diag)
            .collect_vec();
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let file_type = guess_file_type(&uri);
        self.project.write().add_file(
            params.text_document.uri,
            params.content_changes.into_iter().next().unwrap().text,
            file_type,
        );
        let diagnostics = self
            .project
            .read()
            .get_diagnostics(&uri)
            .iter()
            .map(lsp_diag_from_diag)
            .collect_vec();
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {}

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.project.write().remove_file(&params.text_document.uri);
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = position_to_ginko_position(params.text_document_position_params.position);
        let project = self.project.read();
        let Some(item) = project.find_at_pos(&uri, &pos) else {
            return Ok(None);
        };
        match item {
            ItemAtCursor::Reference(reference) => {
                match project.get_node_position(&uri, reference) {
                    Some(span) => Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                        uri.clone(),
                        ginko_span_to_range(span),
                    )))),
                    None => Ok(None),
                }
            }
            ItemAtCursor::Include(include) => {
                match Url::from_file_path(Path::new(include.file_name.item())) {
                    Ok(url) => Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
                        url,
                        Range::new(Position::new(0, 0), Position::new(0, 0)),
                    )))),
                    _ => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = position_to_ginko_position(params.text_document_position_params.position);
        let project = self.project.read();
        let Some(item) = project.find_at_pos(&uri, &pos) else {
            return Ok(None);
        };
        let str = match item {
            ItemAtCursor::Reference(reference) => match project.document_reference(&uri, reference)
            {
                Some(str) => str,
                None => return Ok(None),
            },
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
        let uri = params.text_document.uri;
        let project = self.project.read();
        let Some(root) = project.get_root(&uri) else {
            return Ok(None);
        };
        #[allow(deprecated)]
        fn node_payload_to_symbol(payload: &NodePayload) -> Vec<DocumentSymbol> {
            let mut children: Vec<DocumentSymbol> = Vec::new();
            for prop in &payload.properties {
                children.push(DocumentSymbol {
                    name: prop.name.item().clone(),
                    detail: None,
                    kind: SymbolKind::PROPERTY,
                    tags: None,
                    deprecated: None,
                    range: ginko_span_to_range(prop.span()),
                    selection_range: ginko_span_to_range(prop.name.span()),
                    children: None,
                })
            }
            for node in &payload.child_nodes {
                children.push(node_to_symbol(node))
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
