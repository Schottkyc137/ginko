use crate::dts::lex::token::Token;
use crate::dts::syntax::SyntaxKind;
use rowan::{GreenNode, GreenNodeBuilder};

pub(crate) struct NodeBuilder {
    inner: GreenNodeBuilder<'static>,
}

impl NodeBuilder {
    pub fn new(kind: SyntaxKind) -> NodeBuilder {
        let mut builder = NodeBuilder {
            inner: GreenNodeBuilder::new(),
        };
        builder.inner.start_node(kind.into());
        builder
    }

    pub fn finish(mut self) -> GreenNode {
        self.inner.finish_node();
        self.inner.finish()
    }

    pub fn token(&mut self, token: Token) {
        self.inner.token(token.kind.into(), &token.value)
    }
}
