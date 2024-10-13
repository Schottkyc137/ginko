use crate::dts::ast;
use crate::dts::ast::node::Name;
use crate::dts::eval::Eval;
use crate::dts::model::{NodeName, Path};
use itertools::Itertools;
use std::fmt::{Display, Formatter};

pub enum NodeNameEvalError {}

impl Display for NodeNameEvalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl Eval<NodeName, NodeNameEvalError> for Name {
    fn eval(&self) -> crate::dts::eval::Result<NodeName, NodeNameEvalError> {
        // TODO: check for illegal chars
        let text = self.text();
        if let Some((node_name, address)) = text.split_once('@') {
            Ok(NodeName::with_address(node_name, address))
        } else {
            Ok(NodeName::simple(text))
        }
    }
}

impl Eval<Path, NodeNameEvalError> for ast::cell::Path {
    fn eval(&self) -> crate::dts::eval::Result<Path, NodeNameEvalError> {
        self.items().map(|node_name| node_name.eval()).try_collect()
    }
}
