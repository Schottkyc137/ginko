use crate::dts::analysis::{Analysis, AnalysisContext, ProjectState, PushIntoDiagnostics};
use crate::dts::ast::node as ast;
use crate::dts::ast::node::NodeOrProperty;
use crate::dts::diagnostics::Diagnostic;
use crate::dts::model;
use std::collections::HashMap;

impl Analysis<model::Node> for ast::NodeBody {
    fn analyze(
        &self,
        context: &AnalysisContext,
        project: &ProjectState,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<model::Node, Diagnostic> {
        let mut nodes = HashMap::new();
        let mut properties = HashMap::new();
        for child in self.children() {
            match child {
                NodeOrProperty::Node(node) => {
                    let Some((name, node)) = node
                        .analyze(context, project, diagnostics)
                        .or_push_into(diagnostics)
                    else {
                        continue;
                    };
                    // TODO: duplicates
                    nodes.insert(name, node);
                }
                NodeOrProperty::Property(property) => {
                    let Some((name, property)) = property
                        .analyze(context, project, diagnostics)
                        .or_push_into(diagnostics)
                    else {
                        continue;
                    };
                    // TODO: duplicates
                    properties.insert(name, property);
                }
                NodeOrProperty::DeleteSpec(_) => unimplemented!(),
            }
        }
        Ok(model::Node::new(nodes, properties))
    }
}

impl Analysis<(String, model::Node)> for ast::Node {
    fn analyze(
        &self,
        context: &AnalysisContext,
        project: &ProjectState,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<(String, model::Node), Diagnostic> {
        // TODO: node name unwrapped
        let name = self.name().node_name().unwrap();
        let body = self.body().analyze(context, project, diagnostics)?;
        Ok((name, body))
    }
}

impl Analysis<(String, Vec<model::Value>)> for ast::Property {
    fn analyze(
        &self,
        context: &AnalysisContext,
        project: &ProjectState,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Result<(String, Vec<model::Value>), Diagnostic> {
        // TODO: property name unwrapped
        let name = self.name().property_name().unwrap();
        let value = match self.value() {
            None => vec![],
            Some(value) => value.analyze(context, project, diagnostics)?,
        };
        Ok((name, value))
    }
}

#[cfg(test)]
mod tests {
    use crate::dts::analysis::NoErrorAnalysis;
    use crate::dts::ast::node::Node;
    use crate::dts::model::NodeBuilder;

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

        assert_eq!(node_name, "node".to_owned());
        assert_eq!(
            body,
            NodeBuilder::new()
                .property("#size-cells", 1_u32)
                .node("sub_node", NodeBuilder::new().empty_property("empty_prop"))
                .build()
        );
    }
}
