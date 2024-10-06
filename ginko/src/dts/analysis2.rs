use crate::dts::ast2::{
    AnyDirective, Cell, DtsFile, Include, Node, NodeItem, NodePayload, Path, Primary, Property,
    PropertyValue, Reference, ReferencedNode, WithToken,
};
use crate::dts::data::{HasSource, HasSpan, Span};
use crate::dts::error_codes::ErrorCode;
use crate::dts::import_guard::ImportGuard;
use crate::dts::{Diagnostic2, FileType, Position, Project};
use std::collections::HashMap;
use std::path::{Path as StdPath, PathBuf};
use std::sync::Arc;

/// Something that can be labeled.
/// Used when analyzing a device-tree
#[derive(Clone)]
enum Labeled {
    Node(Arc<Node>),
    #[allow(unused)]
    Property(Arc<Property>),
}

/// Struct containing all important information when analyzing a device-tree.
/// This struct only takes care of identifying cyclic dependencies, but
pub(crate) struct Analysis {
    import_guard: ImportGuard<PathBuf>,
}

impl Analysis {
    pub fn new() -> Analysis {
        Analysis {
            import_guard: ImportGuard::default(),
        }
    }
}

#[derive(Clone)]
pub struct AnalysisContext {
    labels: HashMap<String, Labeled>,
    flat_nodes: HashMap<Path, Arc<Node>>,
}

pub struct AnalysisResult {
    pub context: AnalysisContext,
    pub diagnostics: Vec<Diagnostic2>,
}

impl AnalysisContext {
    pub fn get_node_by_label(&self, label: &str) -> Option<&Arc<Node>> {
        match self.labels.get(label) {
            Some(Labeled::Node(node)) => Some(node),
            _ => None,
        }
    }

    pub fn get_node_by_path(&self, path: &Path) -> Option<&Arc<Node>> {
        self.flat_nodes.get(path)
    }

    pub fn get_referenced(&self, reference: &Reference) -> Option<&Arc<Node>> {
        match reference {
            Reference::Label(label) => self.get_node_by_label(label),
            Reference::Path(path) => self.get_node_by_path(path),
        }
    }
}

pub struct FileContext<'a> {
    project: &'a Project,
    diagnostics: Vec<Diagnostic2>,
    labels: HashMap<String, Labeled>,
    flat_nodes: HashMap<Path, Arc<Node>>,
    unresolved_references: Vec<WithToken<Reference>>,
    file_type: FileType,
    is_plugin: bool,
    first_non_include: bool,
    dts_header_seen: bool,
}

impl FileContext<'_> {
    pub fn add_diagnostic(&mut self, diagnostic: Diagnostic2) {
        self.diagnostics.push(diagnostic);
    }

    pub fn resolve_reference(&self, label: &String) -> Option<(&Path, &Arc<Node>)> {
        self.flat_nodes
            .iter()
            .find(|(_, value)| value.label.as_ref().map(|node| node.item()) == Some(label))
    }
}

impl FileContext<'_> {
    pub fn into_result(self) -> AnalysisResult {
        AnalysisResult {
            context: AnalysisContext {
                flat_nodes: self.flat_nodes,
                labels: self.labels,
            },
            diagnostics: self.diagnostics,
        }
    }
}

impl Analysis {
    pub fn analyze_file(
        &mut self,
        file: &DtsFile,
        file_type: FileType,
        project: &Project,
    ) -> AnalysisResult {
        let mut ctx = FileContext {
            file_type,
            labels: HashMap::default(),
            diagnostics: Vec::default(),
            flat_nodes: HashMap::default(),
            unresolved_references: Vec::default(),
            project,
            is_plugin: file_type == FileType::DtSourceOverlay,
            dts_header_seen: false,
            first_non_include: false,
        };
        for primary in &file.elements {
            match primary {
                Primary::Directive(directive) => match directive {
                    AnyDirective::DtsHeader(tok) => {
                        if ctx.dts_header_seen {
                            ctx.add_diagnostic(Diagnostic2::from_token(
                                tok.clone(),
                                ErrorCode::DuplicateDirective,
                                "Duplicate dts-v1 version header",
                            ))
                        } else if ctx.first_non_include {
                            ctx.add_diagnostic(Diagnostic2::from_token(
                                tok.clone(),
                                ErrorCode::MisplacedDtsHeader,
                                "dts-v1 header must be placed on top of the file",
                            ))
                        }
                        ctx.dts_header_seen = true;
                    }
                    AnyDirective::Memreserve(_) => ctx.first_non_include = true,
                    AnyDirective::Include(include) => self.analyze_include(&mut ctx, file, include),
                    AnyDirective::Plugin(_) => {
                        ctx.first_non_include = true;
                        ctx.is_plugin = true
                    }
                    AnyDirective::OmitIfNoRef(..) => ctx.first_non_include = true,
                    AnyDirective::DeletedNode(_, reference) => {
                        self.resolve_reference(&mut ctx, reference);
                    }
                },
                Primary::Root(root_node) => {
                    self.analyze_node(&mut ctx, root_node.clone(), Path::empty());
                    ctx.first_non_include = true
                }
                Primary::ReferencedNode(referenced_node) => {
                    self.analyze_referenced_node(&mut ctx, referenced_node);
                    ctx.first_non_include = true
                }
                Primary::CStyleInclude(_) => {}
            }
        }
        if !ctx.dts_header_seen && ctx.file_type == FileType::DtSource {
            ctx.add_diagnostic(Diagnostic2::new(
                Position::zero().as_span(),
                file.source(),
                ErrorCode::NonDtsV1,
                "Files without the '/dts-v1/' Header are not supported",
            ))
        }
        self.resolve_references(&mut ctx);
        ctx.into_result()
    }

    fn analyze_include(&mut self, ctx: &mut FileContext<'_>, parent: &DtsFile, include: &Include) {
        let path = match include.path() {
            Ok(path) => path,
            Err(err) => {
                ctx.add_diagnostic(Diagnostic2::io_error(include.span(), include.source(), err));
                return;
            }
        };
        if let Err(err) = self
            .import_guard
            .add(path.clone(), &[parent.source.clone().to_path_buf()])
        {
            ctx.add_diagnostic(Diagnostic2::cyclic_dependency_error(
                include.span(),
                include.source(),
                err,
            ));
            return;
        }
        let Some(proj_file) = ctx.project.get_file(&path) else {
            return;
        };
        if proj_file.has_errors(&ctx.project.severities) {
            ctx.add_diagnostic(Diagnostic2::new(
                include.span(),
                include.source(),
                ErrorCode::ErrorsInInclude,
                "Included file contains errors",
            ));
        }
        if let Some(context) = proj_file.context.as_ref() {
            ctx.flat_nodes.extend(context.flat_nodes.clone());
            ctx.labels.extend(context.labels.clone());
        }
    }

    fn unresolved_reference_error(
        &self,
        ctx: &mut FileContext<'_>,
        span: Span,
        source: Arc<StdPath>,
    ) {
        // Do not emit unresolved reference errors when we are not a plugin.
        // This will emit false positives as references can only be resolved with the full
        // device-tree information.
        if ctx.file_type == FileType::DtSource && !ctx.is_plugin {
            ctx.add_diagnostic(Diagnostic2::new(
                span,
                source,
                ErrorCode::UnresolvedReference,
                "Reference cannot be resolved",
            ));
        }
    }

    fn resolve_reference(
        &mut self,
        ctx: &mut FileContext<'_>,
        reference: &WithToken<Reference>,
    ) -> Path {
        match reference.item() {
            Reference::Label(label) => match ctx.resolve_reference(label) {
                None => {
                    self.unresolved_reference_error(ctx, reference.span(), reference.source());
                    Path::empty()
                }
                Some((path, _)) => path.clone(),
            },
            Reference::Path(path) => {
                if !ctx.flat_nodes.contains_key(path) {
                    self.unresolved_reference_error(ctx, reference.span(), reference.source());
                };
                path.clone()
            }
        }
    }

    pub fn analyze_referenced_node(&mut self, ctx: &mut FileContext<'_>, node: &ReferencedNode) {
        let path = if ctx.file_type == FileType::DtSource {
            self.resolve_reference(ctx, &node.reference)
        } else {
            // This is an include; simply assume the 'root' path
            Path::empty()
        };
        self.analyze_node_payload(ctx, &node.payload, path);
    }

    pub fn resolve_references(&self, ctx: &mut FileContext<'_>) {
        for reference in ctx.unresolved_references.clone() {
            let span = reference.span();
            let source = reference.source();
            match &reference.item() {
                Reference::Label(label) => match ctx.labels.get(label) {
                    Some(_) => {}
                    None => {
                        self.unresolved_reference_error(ctx, span, source);
                    }
                },
                Reference::Path(path) => match ctx.flat_nodes.get(path) {
                    None => {
                        self.unresolved_reference_error(ctx, span, source);
                    }
                    Some(_) => {}
                },
            }
        }
    }

    pub fn analyze_node(&mut self, ctx: &mut FileContext<'_>, node: Arc<Node>, path: Path) {
        if let Some(label) = &node.label {
            ctx.labels
                .insert(label.item().clone(), Labeled::Node(node.clone()));
        }
        ctx.flat_nodes.insert(path.clone(), node.clone());
        self.analyze_node_payload(ctx, &node.payload, path)
    }

    fn analyze_node_payload(
        &mut self,
        ctx: &mut FileContext<'_>,
        payload: &NodePayload,
        path: Path,
    ) {
        for item in &payload.items {
            match item {
                NodeItem::Property(property) => self.analyze_property(ctx, property.clone()),
                NodeItem::Node(node) => {
                    self.analyze_node(ctx, node.clone(), path.with_child(node.name.item().clone()))
                }
                NodeItem::DeletedNode(..) => {}
                NodeItem::DeletedProperty(..) => {}
            }
        }
    }

    fn check_is_string_list(&mut self, ctx: &mut FileContext<'_>, values: &Vec<PropertyValue>) {
        for value in values {
            if !matches!(value, PropertyValue::String(_)) {
                ctx.add_diagnostic(Diagnostic2::new(
                    value.span(),
                    value.source(),
                    ErrorCode::NonStringInCompatible,
                    "compatible property should only contain strings",
                ))
            }
        }
    }

    fn check_is_single_string(&mut self, _ctx: &mut FileContext<'_>, values: &[PropertyValue]) {
        if values.len() != 1 {}
    }

    fn check_is_single_u32(&mut self, _ctx: &mut FileContext<'_>, values: &[PropertyValue]) {
        if values.len() != 1 {}
    }

    pub fn analyze_property(&mut self, ctx: &mut FileContext<'_>, property: Arc<Property>) {
        if let Some(label) = &property.label {
            ctx.labels
                .insert(label.item().clone(), Labeled::Property(property.clone()));
        }
        for value in &property.values {
            self.analyze_property_value(ctx, value)
        }

        match property.name.as_str() {
            "compatible" => self.check_is_string_list(ctx, &property.values),
            "model" => self.check_is_single_string(ctx, &property.values),
            "phandle" => self.check_is_single_u32(ctx, &property.values),
            _ => {}
        }
    }

    pub fn analyze_property_value(&mut self, ctx: &mut FileContext<'_>, value: &PropertyValue) {
        match value {
            PropertyValue::String(_) => {}
            PropertyValue::ByteStrings(..) => {}
            PropertyValue::Cells(_, cells, _) => {
                for cell in cells {
                    self.analyze_cell(ctx, cell)
                }
            }
            PropertyValue::Reference(reference) => self.analyze_reference(ctx, reference),
        }
    }

    pub fn analyze_cell(&mut self, ctx: &mut FileContext<'_>, value: &Cell) {
        match value {
            Cell::Number(_) => {}
            Cell::Reference(reference) => self.analyze_reference(ctx, reference),
            Cell::Expression => {}
        }
    }

    pub fn analyze_reference(
        &mut self,
        ctx: &mut FileContext<'_>,
        reference: &WithToken<Reference>,
    ) {
        ctx.unresolved_references.push(reference.clone())
    }
}

#[cfg(test)]
mod test {
    use crate::dts::ast2::Path;
    use crate::dts::data::{HasSource, HasSpan, Position};
    use crate::dts::error_codes::ErrorCode;
    use crate::dts::test::Code;
    use crate::dts::Diagnostic2;
    use assert_unordered::assert_eq_unordered;

    #[test]
    pub fn test_illegal_char_in_label() {
        let code = Code::new(
            "\
/dts-v1/;

/{
    my_l?abel: some_node {};
    my_label_that_has_more_than_31_characters: other_node {};
    some_other_node {
        another_ill#gal_label: sub_node {};
    };
    illegal_node_name#s {};
};",
        );
        let (diagnostics, _) = code.get_analyzed_file();
        assert_eq_unordered!(
            diagnostics,
            vec![
                Diagnostic2::new(
                    Position::new(8, 21).as_char_span(),
                    code.source(),
                    ErrorCode::IllegalChar,
                    "Illegal char '#' in node name"
                ),
                Diagnostic2::new(
                    Position::new(3, 8).as_char_span(),
                    code.source(),
                    ErrorCode::IllegalChar,
                    "Illegal char '?' in label"
                ),
                Diagnostic2::new(
                    Position::new(4, 4).char_to(46),
                    code.source(),
                    ErrorCode::NameTooLong,
                    "label should only have 31 characters but has 41 characters"
                ),
                Diagnostic2::new(
                    Position::new(6, 19).as_char_span(),
                    code.source(),
                    ErrorCode::IllegalChar,
                    "Illegal char '#' in label"
                ),
            ]
        )
    }

    #[test]
    pub fn test_resolve_node_names() {
        let code = Code::new(
            "\
/dts-v1/;

/{
    node1: some_node {
        ref-to-node2 = &node2;
        ref-to-node3 = <&node3>;
    };
    node2: some_other_node {
        ref-to-node1 = &node1;
        ref-to-node1-path = &{/some_node};
        ref-to-node4-path = &{/some_other_node/some_node};
        ref-to-node3-path = &{/node3};
        node4: some_node {
            self-reference = &node4;
        };
    };
};",
        );
        let (diagnostics, context) = code.get_analyzed_file();
        assert_eq_unordered!(
            diagnostics,
            vec![
                Diagnostic2::new(
                    code.s1("&node3").span(),
                    code.source(),
                    ErrorCode::UnresolvedReference,
                    "Reference cannot be resolved"
                ),
                Diagnostic2::new(
                    code.s1("&{/node3}").span(),
                    code.source(),
                    ErrorCode::UnresolvedReference,
                    "Reference cannot be resolved"
                ),
            ]
        );
        assert_eq!(
            context
                .get_node_by_label("node1")
                .expect("Reference should be set")
                .name
                .span(),
            code.s1("some_node").span()
        );
        assert_eq!(
            context
                .get_node_by_label("node2")
                .expect("Reference should be set")
                .name
                .span(),
            code.s1("some_other_node").span(),
        );
        assert_eq!(
            context
                .get_node_by_label("node4")
                .expect("Reference should be set")
                .name
                .span(),
            code.s1("node4: some_node").s1("some_node").span()
        );
        assert!(context.get_node_by_label("node3").is_none())
    }

    #[test]
    pub fn test_resolve_node_paths() {
        let code = Code::new(
            "\
/dts-v1/;

/{
    node1: some_node {
        ref-to-node2 = &node2;
    };
    node2: some_other_node {
        ref-to-node1 = &node1;
        node4: some_node {
            self-reference = &node4;
        };
    };
};",
        );
        let (diag, context) = code.get_analyzed_file();
        assert!(diag.is_empty());
        assert_eq!(
            context
                .get_node_by_path(&Path::new(vec!["some_node".into()]))
                .expect("Reference should be set")
                .name
                .span(),
            code.s1("some_node").span(),
        );
        assert_eq!(
            context
                .get_node_by_path(&Path::new(vec!["some_other_node".into()]))
                .expect("Reference should be set")
                .name
                .span(),
            code.s1("some_other_node").span(),
        );
        assert_eq!(
            context
                .get_node_by_path(&Path::new(vec![
                    "some_other_node".into(),
                    "some_node".into(),
                ]))
                .expect("Reference should be set")
                .name
                .span(),
            code.s1("node4: some_node").s1("some_node").span()
        );
        assert!(context.get_node_by_label("node3").is_none())
    }

    #[test]
    pub fn test_does_not_accept_non_dtsv1_sources() {
        let code = Code::new("/ {};");
        let (diagnostics, _) = code.get_analyzed_file();
        assert_eq!(
            diagnostics,
            vec![Diagnostic2::new(
                Position::zero().as_span(),
                code.source(),
                ErrorCode::NonDtsV1,
                "Files without the '/dts-v1/' Header are not supported"
            )]
        )
    }

    #[test]
    pub fn referenced_node_in_same_file() {
        let code = Code::new(
            "\
/dts-v1/;

/ {
    some_node: node {};
};

&some_node {};

&some_other_node {};

&{/node} {};

&{/some_other_node} {};

",
        );
        let (diagnostics, _) = code.get_analyzed_file();
        assert_eq_unordered!(
            diagnostics,
            vec![
                Diagnostic2::new(
                    code.s1("&some_other_node").span(),
                    code.source(),
                    ErrorCode::UnresolvedReference,
                    "Reference cannot be resolved"
                ),
                Diagnostic2::new(
                    code.s1("&{/some_other_node}").span(),
                    code.source(),
                    ErrorCode::UnresolvedReference,
                    "Reference cannot be resolved"
                )
            ]
        )
    }
}
