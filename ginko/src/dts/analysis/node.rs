use crate::dts::analysis::{Analyzer, PushIntoDiagnostics};
use crate::dts::ast::node as ast;
use crate::dts::ast::node::{Node, NodeBody, NodeOrProperty};
use crate::dts::diagnostics::Diagnostic;
use crate::dts::eval::Eval;
use crate::dts::model::NodeName;
use crate::dts::{model, ErrorCode};
use std::collections::HashMap;

impl Analyzer {
    pub fn analyze_node_body(
        &self,
        node: &NodeBody,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<model::Node, Diagnostic> {
        let mut nodes = HashMap::new();
        let mut properties = HashMap::new();
        for child in node.children() {
            match child {
                NodeOrProperty::Node(node) => {
                    let Some((name, node)) = self
                        .analyze_node(&node, diagnostics)
                        .or_push_into(diagnostics)
                    else {
                        continue;
                    };
                    // TODO: duplicates
                    nodes.insert(name, node);
                }
                NodeOrProperty::Property(property) => {
                    let (name, property) = self.analyze_property(&property, diagnostics);
                    // TODO: duplicates
                    properties.insert(name, property);
                }
                // we just ignore this for now
                NodeOrProperty::DeleteSpec(_) => {}
            }
        }
        Ok(model::Node::new(nodes, properties))
    }

    pub fn analyze_node(
        &self,
        node: &Node,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<(NodeName, model::Node), Diagnostic> {
        let name = node.name().eval()?;
        let body = self.analyze_node_body(&node.body(), diagnostics)?;
        Ok((name, body))
    }

    pub fn analyze_property(
        &self,
        property: &ast::Property,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> (String, Vec<model::Value>) {
        let name = property.name();
        let prop_name = match name.property_name() {
            Ok(name) => name,
            Err(err) => {
                diagnostics.push(Diagnostic::new(
                    name.range(),
                    ErrorCode::ExpectedName,
                    err.to_string(),
                ));
                name.text()
            }
        };
        let value = match property.value() {
            None => vec![],
            Some(value) => self.analyze_property_list(&value, diagnostics),
        };
        (prop_name, value)
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::analysis::{
        Analyzer, NoErrorAnalysis, PushIntoDiagnostics, WithDiagnosticAnalysis,
    };
    use crate::dts::ast::node::Node;
    use crate::dts::diagnostics::Diagnostic;
    use crate::dts::model;
    use crate::dts::model::NodeBuilder;

    impl WithDiagnosticAnalysis<(model::NodeName, model::Node)> for Node {
        fn analyze_with_diagnostics(
            &self,
        ) -> (Option<(model::NodeName, model::Node)>, Vec<Diagnostic>) {
            let analyzer = Analyzer::new();
            let mut diagnostics = Vec::new();
            let value = analyzer
                .analyze_node(self, &mut diagnostics)
                .or_push_into(&mut diagnostics);
            (value, diagnostics)
        }
    }

    #[test]
    fn node_value() {
        let (node_name, body) = "\
node {
  #size-cells = <1>;
  sub_node {
    empty_prop;
  };
};"
        .parse::<Node>()
        .unwrap()
        .analyze_no_errors();

        assert_eq!(node_name, "node".into());
        assert_eq!(
            body,
            NodeBuilder::new()
                .property("#size-cells", 1_u32)
                .node("sub_node", NodeBuilder::new().empty_property("empty_prop"))
                .build()
        );
    }
}
