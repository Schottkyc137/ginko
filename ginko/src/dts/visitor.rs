use crate::dts::ast::{
    AnyDirective, Cell, DtsFile, Include, Node, NodePayload, Primary, Property, PropertyValue,
    Reference, ReferencedNode, WithToken,
};
use crate::dts::{HasSpan, NodeItem, Position};

#[allow(unused)]
pub enum ItemAtCursor<'a> {
    Property(&'a Property),
    Node(&'a Node),
    Reference(&'a Reference),
    Label(&'a WithToken<String>),
    Include(&'a Include),
}

impl DtsFile {
    pub fn item_at_cursor(&self, cursor: &Position) -> Option<ItemAtCursor> {
        for element in &self.elements {
            if let Some(item) = element.item_at_cursor(cursor) {
                return Some(item);
            }
        }
        None
    }
}

impl Primary {
    pub fn item_at_cursor(&self, cursor: &Position) -> Option<ItemAtCursor> {
        match self {
            Primary::Directive(directive) => directive.item_at_cursor(cursor),
            Primary::Root(root) => root.item_at_cursor(cursor),
            Primary::ReferencedNode(node) => node.item_at_cursor(cursor),
            Primary::CStyleInclude(_) => None,
        }
    }
}

impl AnyDirective {
    pub fn item_at_cursor(&self, cursor: &Position) -> Option<ItemAtCursor> {
        match self {
            AnyDirective::Include(include) => {
                if include.span().contains(cursor) {
                    Some(ItemAtCursor::Include(include))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

impl ReferencedNode {
    pub fn item_at_cursor(&self, cursor: &Position) -> Option<ItemAtCursor> {
        if self.reference.span().contains(cursor) {
            return Some(ItemAtCursor::Reference(self.reference.item()));
        }
        self.payload.item_at_cursor(cursor)
    }
}

impl Node {
    pub fn item_at_cursor(&self, cursor: &Position) -> Option<ItemAtCursor> {
        if let Some(label) = &self.label {
            if label.span().contains(cursor) {
                return Some(ItemAtCursor::Label(label));
            }
        }
        self.payload.item_at_cursor(cursor)
    }
}

impl NodePayload {
    pub fn item_at_cursor(&self, cursor: &Position) -> Option<ItemAtCursor> {
        for node in &self.items {
            if let Some(item) = node.item_at_cursor(cursor) {
                return Some(item);
            }
        }
        None
    }
}

impl NodeItem {
    pub fn item_at_cursor(&self, cursor: &Position) -> Option<ItemAtCursor> {
        match self {
            NodeItem::Property(property) => property.item_at_cursor(cursor),
            NodeItem::Node(node) => node.item_at_cursor(cursor),
            NodeItem::DeletedNode(_, _) => None,
            NodeItem::DeletedProperty(_, _) => None,
        }
    }
}

impl Property {
    pub fn item_at_cursor(&self, cursor: &Position) -> Option<ItemAtCursor> {
        if let Some(label) = &self.label {
            if label.span().contains(cursor) {
                return Some(ItemAtCursor::Label(label));
            }
        }
        for value in &self.values {
            if let Some(item) = value.item_at_cursor(cursor) {
                return Some(item);
            }
        }
        None
    }
}

impl PropertyValue {
    pub fn item_at_cursor(&self, cursor: &Position) -> Option<ItemAtCursor> {
        match self {
            PropertyValue::String(_) => None,
            PropertyValue::Cells(_, cells, _) => {
                for cell in cells {
                    match cell {
                        Cell::Number(_) | Cell::Expression => {}
                        Cell::Reference(reference) => {
                            if reference.span().contains(cursor) {
                                return Some(ItemAtCursor::Reference(reference.item()));
                            }
                        }
                    }
                }
                None
            }
            PropertyValue::Reference(reference) => {
                if reference.span().contains(cursor) {
                    Some(ItemAtCursor::Reference(reference.item()))
                } else {
                    None
                }
            }
            PropertyValue::ByteStrings(..) => None,
        }
    }
}
