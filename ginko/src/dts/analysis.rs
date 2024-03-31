use crate::dts::ast::{
    AnyDirective, Cell, DtsFile, Include, Node, NodePayload, Path, Primary, Property,
    PropertyValue, Reference, ReferencedNode, WithToken,
};
use crate::dts::data::{HasSource, HasSpan, Span};
use crate::dts::diagnostics::DiagnosticKind;
use crate::dts::project::FileManager;
use crate::dts::{CompilerDirective, Diagnostic, FileType, Position};
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

/// Struct containing all important information when analyzing a device-tree
pub struct Analysis<'a> {
    project: &'a mut dyn FileManager,
    labels: HashMap<String, Labeled>,
    flat_nodes: HashMap<Path, Arc<Node>>,
    unresolved_references: Vec<WithToken<Reference>>,
    file_type: FileType,
    is_plugin: bool,
}

impl<'a> Analysis<'a> {
    pub fn new(file_type: FileType, project: &'a mut dyn FileManager) -> Analysis<'a> {
        Analysis {
            project,
            labels: Default::default(),
            flat_nodes: Default::default(),
            unresolved_references: Default::default(),
            file_type,
            is_plugin: false,
        }
    }
}

pub struct AnalysisContext {
    labels: HashMap<String, Labeled>,
    flat_nodes: HashMap<Path, Arc<Node>>,
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

impl Analysis<'_> {
    pub fn into_context(self) -> AnalysisContext {
        AnalysisContext {
            flat_nodes: self.flat_nodes,
            labels: self.labels,
        }
    }
}

impl Analysis<'_> {
    pub fn analyze_file(&mut self, diagnostics: &mut Vec<Diagnostic>, file: &DtsFile) {
        let mut first_non_include = false;
        let mut dts_header_seen = false;
        for primary in &file.elements {
            match primary {
                Primary::Directive(directive) => match directive {
                    AnyDirective::DtsHeader(tok) => {
                        if dts_header_seen {
                            diagnostics.push(Diagnostic::from_token(
                                tok.clone(),
                                DiagnosticKind::DuplicateDirective(
                                    CompilerDirective::DTSVersionHeader,
                                ),
                            ))
                        } else if first_non_include {
                            diagnostics.push(Diagnostic::from_token(
                                tok.clone(),
                                DiagnosticKind::MisplacedDtsHeader,
                            ))
                        }
                        dts_header_seen = true;
                    }
                    AnyDirective::Memreserve(_) => first_non_include = true,
                    AnyDirective::Include(include) => {
                        self.analyze_include(file, diagnostics, include)
                    }
                    AnyDirective::Plugin(_) => {
                        first_non_include = true;
                        self.is_plugin = true
                    }
                },
                Primary::Root(root_node) => {
                    self.analyze_node(diagnostics, root_node.clone(), Path::empty());
                    first_non_include = true
                }
                Primary::ReferencedNode(referenced_node) => {
                    self.analyze_referenced_node(diagnostics, referenced_node);
                    first_non_include = true
                }
                Primary::CStyleInclude(_) => {}
            }
        }
        if !dts_header_seen && self.file_type == FileType::DtSource {
            diagnostics.push(Diagnostic::new(
                Position::zero().as_span(),
                file.source(),
                DiagnosticKind::NonDtsV1,
            ))
        }
        self.resolve_references(diagnostics)
    }

    fn analyze_include(
        &mut self,
        parent: &DtsFile,
        diagnostics: &mut Vec<Diagnostic>,
        include: &Include,
    ) {
        let path = include.path();
        let file = match self.project.get_file(path.as_path()) {
            Some(file) => file,
            None => {
                let text = match std::fs::read_to_string(&path) {
                    Ok(text) => text,
                    Err(e) => {
                        diagnostics.push(Diagnostic::new(
                            include.span(),
                            include.include_token.source(),
                            DiagnosticKind::from(e),
                        ));
                        return;
                    }
                };
                let file_type = FileType::from(path.as_path());
                match self.project.add_file_with_parent(
                    path,
                    Some(PathBuf::from(parent.source.as_ref())),
                    text,
                    file_type,
                ) {
                    Ok(file) => file,
                    Err(dependency_error) => {
                        diagnostics.push(Diagnostic::new(
                            include.span(),
                            include.source(),
                            DiagnosticKind::from(dependency_error),
                        ));
                        return;
                    }
                }
            }
        };

        // Analyse file
        if let Some(context) = file.analysis_context() {
            self.labels.extend(context.labels.clone());
            self.flat_nodes.extend(context.flat_nodes.clone());
        }
        if file.has_errors() {
            diagnostics.push(Diagnostic::new(
                include.span(),
                include.source(),
                DiagnosticKind::ErrorsInInclude,
            ))
        }
    }

    fn unresolved_reference_error(
        &self,
        span: Span,
        source: Arc<StdPath>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Do not emit unresolved reference errors when we are not a plugin.
        // This will emit false positives as references can only be resolved with the full
        // device-tree information.
        if self.file_type == FileType::DtSource && !self.is_plugin {
            diagnostics.push(Diagnostic::new(
                span,
                source,
                DiagnosticKind::UnresolvedReference,
            ));
        }
    }

    fn resolve_reference(
        &mut self,
        diagnostics: &mut Vec<Diagnostic>,
        reference: &WithToken<Reference>,
    ) -> Path {
        match reference.item() {
            Reference::Label(label) => {
                let path = self
                    .flat_nodes
                    .iter()
                    .find(|(_, value)| value.label.as_ref().map(|node| node.item()) == Some(label));
                match path {
                    None => {
                        self.unresolved_reference_error(
                            reference.span(),
                            reference.source(),
                            diagnostics,
                        );
                        Path::empty()
                    }
                    Some((path, _)) => path.clone(),
                }
            }
            Reference::Path(path) => {
                if !self.flat_nodes.contains_key(path) {
                    self.unresolved_reference_error(
                        reference.span(),
                        reference.source(),
                        diagnostics,
                    );
                };
                path.clone()
            }
        }
    }

    pub fn analyze_referenced_node(
        &mut self,
        diagnostics: &mut Vec<Diagnostic>,
        node: &ReferencedNode,
    ) {
        let path = if self.file_type == FileType::DtSource {
            self.resolve_reference(diagnostics, &node.reference)
        } else {
            // This is an include; simply assume the 'root' path
            Path::empty()
        };
        self.analyze_node_payload(diagnostics, &node.payload, path);
    }

    pub fn resolve_references(&mut self, diagnostics: &mut Vec<Diagnostic>) {
        for reference in &self.unresolved_references {
            let span = reference.span();
            let source = reference.source();
            match &reference.item() {
                Reference::Label(label) => match self.labels.get(label) {
                    Some(_) => {}
                    None => {
                        self.unresolved_reference_error(span, source, diagnostics);
                    }
                },
                Reference::Path(path) => match self.flat_nodes.get(path) {
                    None => {
                        self.unresolved_reference_error(span, source, diagnostics);
                    }
                    Some(_) => {}
                },
            }
        }
    }

    pub fn analyze_node(&mut self, diagnostics: &mut Vec<Diagnostic>, node: Arc<Node>, path: Path) {
        if let Some(label) = &node.label {
            self.labels
                .insert(label.item().clone(), Labeled::Node(node.clone()));
        }
        self.flat_nodes.insert(path.clone(), node.clone());
        self.analyze_node_payload(diagnostics, &node.payload, path)
    }

    fn analyze_node_payload(
        &mut self,
        diagnostics: &mut Vec<Diagnostic>,
        payload: &NodePayload,
        path: Path,
    ) {
        for prop in payload.properties.clone() {
            self.analyze_property(diagnostics, prop);
        }
        for node in &payload.child_nodes {
            self.analyze_node(
                diagnostics,
                node.clone(),
                path.with_child(node.name.item().clone()),
            )
        }
    }

    fn check_is_string_list(
        &mut self,
        diagnostics: &mut Vec<Diagnostic>,
        values: &Vec<PropertyValue>,
    ) {
        for value in values {
            if !matches!(value, PropertyValue::String(_)) {
                diagnostics.push(Diagnostic::new(
                    value.span(),
                    value.source(),
                    DiagnosticKind::NonStringInCompatible,
                ))
            }
        }
    }

    fn check_is_single_string(
        &mut self,
        _diagnostics: &mut [Diagnostic],
        values: &[PropertyValue],
    ) {
        if values.len() != 1 {}
    }

    fn check_is_single_u32(&mut self, _diagnostics: &mut [Diagnostic], values: &[PropertyValue]) {
        if values.len() != 1 {}
    }

    pub fn analyze_property(&mut self, diagnostics: &mut Vec<Diagnostic>, property: Arc<Property>) {
        if let Some(label) = &property.label {
            self.labels
                .insert(label.item().clone(), Labeled::Property(property.clone()));
        }
        for value in &property.values {
            self.analyze_property_value(diagnostics, value)
        }

        match property.name.as_str() {
            "compatible" => self.check_is_string_list(diagnostics, &property.values),
            "model" => self.check_is_single_string(diagnostics, &property.values),
            "phandle" => self.check_is_single_u32(diagnostics, &property.values),
            _ => {}
        }
    }

    pub fn analyze_property_value(
        &mut self,
        diagnostics: &mut [Diagnostic],
        value: &PropertyValue,
    ) {
        match value {
            PropertyValue::String(_) => {}
            PropertyValue::ByteStrings(..) => {}
            PropertyValue::Cells(_, cells, _) => {
                for cell in cells {
                    self.analyze_cell(diagnostics, cell)
                }
            }
            PropertyValue::Reference(reference) => self.analyze_reference(diagnostics, reference),
        }
    }

    pub fn analyze_cell(&mut self, diagnostics: &mut [Diagnostic], value: &Cell) {
        match value {
            Cell::Number(_) => {}
            Cell::Reference(reference) => self.analyze_reference(diagnostics, reference),
            Cell::Expression => {}
        }
    }

    pub fn analyze_reference(
        &mut self,
        _diagnostics: &mut [Diagnostic],
        reference: &WithToken<Reference>,
    ) {
        self.unresolved_references.push(reference.clone())
    }
}

#[cfg(test)]
mod test {
    use crate::dts::ast::Path;
    use crate::dts::data::{HasSource, HasSpan, Position};
    use crate::dts::diagnostics::{DiagnosticKind, NameContext};
    use crate::dts::test::Code;
    use crate::dts::Diagnostic;
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
                Diagnostic::new(
                    Position::new(8, 21).as_char_span(),
                    code.source(),
                    DiagnosticKind::IllegalChar('#', NameContext::NodeName),
                ),
                Diagnostic::new(
                    Position::new(3, 8).as_char_span(),
                    code.source(),
                    DiagnosticKind::IllegalChar('?', NameContext::Label),
                ),
                Diagnostic::new(
                    Position::new(4, 4).char_to(46),
                    code.source(),
                    DiagnosticKind::NameTooLong(41, NameContext::Label),
                ),
                Diagnostic::new(
                    Position::new(6, 19).as_char_span(),
                    code.source(),
                    DiagnosticKind::IllegalChar('#', NameContext::Label),
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
                Diagnostic::new(
                    code.s1("&node3").span(),
                    code.source(),
                    DiagnosticKind::UnresolvedReference,
                ),
                Diagnostic::new(
                    code.s1("&{/node3}").span(),
                    code.source(),
                    DiagnosticKind::UnresolvedReference,
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
            vec![Diagnostic::new(
                Position::zero().as_span(),
                code.source(),
                DiagnosticKind::NonDtsV1,
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
                Diagnostic::new(
                    code.s1("&some_other_node").span(),
                    code.source(),
                    DiagnosticKind::UnresolvedReference,
                ),
                Diagnostic::new(
                    code.s1("&{/some_other_node}").span(),
                    code.source(),
                    DiagnosticKind::UnresolvedReference,
                )
            ]
        )
    }
}
