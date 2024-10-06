mod display;

use std::cell::OnceCell;
use std::collections::HashMap;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CellValue<T> {
    Number(T),
    Reference(OnceCell<Node>),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CellValues {
    U8(Vec<CellValue<u8>>),
    U16(Vec<CellValue<u16>>),
    U32(Vec<CellValue<u32>>),
    U64(Vec<CellValue<u64>>),
}

macro_rules! cell_values_from_iter {
    ($($t:ident => $target:expr),+) => {
        $(
            impl FromIterator<CellValue<$t>> for CellValues {
                fn from_iter<T: IntoIterator<Item = CellValue<$t>>>(iter: T) -> Self {
                    $target(Vec::from_iter(iter))
                }
            }
        )+
    };
}

cell_values_from_iter! {
    u8 => CellValues::U8,
    u16 => CellValues::U16,
    u32 => CellValues::U32,
    u64 => CellValues::U64
}

macro_rules! cell_values_from_int {
    ($($t:ident => $target:expr),+) => {
        $(
            impl From<$t> for CellValues {
                fn from(value: $t) -> Self {
                    $target(vec![CellValue::Number(value)])
                }
            }
        )+
    };
}

cell_values_from_int! {
    u8 => CellValues::U8,
    u16 => CellValues::U16,
    u32 => CellValues::U32,
    u64 => CellValues::U64
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Value {
    Bytes(Vec<u8>),
    String(String),
    Cell(CellValues),
    Reference(OnceCell<Node>),
}

macro_rules! value_from_int {
    ($($t:ident),+) => {
        $(
            impl From<$t> for Value {
                fn from(value: $t) -> Self {
                    Value::Cell(value.into())
                }
            }
        )+
    };
}

value_from_int!(u8, u16, u32, u64);

impl<const N: usize> From<[u8; N]> for Value {
    fn from(value: [u8; N]) -> Self {
        Value::Bytes(value.to_vec())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::String(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::String(value.to_owned())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct Node {
    nodes: HashMap<String, Node>,
    properties: HashMap<String, Vec<Value>>,
}

impl Node {
    pub fn merge(&mut self, other: Node) {
        for (name, incoming_node) in other.nodes {
            if let Some(own_node) = self.nodes.get_mut(&name) {
                // merge the node (possibly deep), if it already exists
                own_node.merge(incoming_node)
            } else {
                // else simply insert
                self.nodes.insert(name, incoming_node);
            }
        }
        // TODO: warn on duplicates?
        self.properties.extend(other.properties);
    }
}

impl Node {
    pub fn new(nodes: HashMap<String, Node>, properties: HashMap<String, Vec<Value>>) -> Node {
        Node { nodes, properties }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReservedMemory {
    pub address: u64,
    pub length: u64,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct File {
    reserved_memory: Vec<ReservedMemory>,
    root: Node,
}

impl File {
    pub fn new(root: Node, reserved_memory: Vec<ReservedMemory>) -> File {
        File {
            root,
            reserved_memory,
        }
    }

    pub fn root(&self) -> &Node {
        &self.root
    }

    pub fn reserved_memory(&self) -> &[ReservedMemory] {
        &self.reserved_memory
    }
}

pub struct NodeBuilder {
    nodes: HashMap<String, Node>,
    properties: HashMap<String, Vec<Value>>,
}

impl NodeBuilder {
    pub fn new() -> NodeBuilder {
        NodeBuilder {
            nodes: HashMap::default(),
            properties: HashMap::default(),
        }
    }

    pub fn property(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.properties.insert(name.into(), vec![value.into()]);
        self
    }

    pub fn empty_property(mut self, name: impl Into<String>) -> Self {
        self.properties.insert(name.into(), vec![]);
        self
    }

    pub fn node(mut self, name: impl Into<String>, value: impl Into<Node>) -> Self {
        self.nodes.insert(name.into(), value.into());
        self
    }

    pub fn build(self) -> Node {
        Node::new(self.nodes, self.properties)
    }
}

impl<'a> From<NodeBuilder> for Node {
    fn from(value: NodeBuilder) -> Self {
        value.build()
    }
}

#[test]
fn merge_nodes() {
    // / { some_node { prop_1 = <17>; } }
    let mut node_a = NodeBuilder::new()
        .node("some_node", NodeBuilder::new().property("prop_1", 17_u32))
        .build();
    // / { some_node { prop_2 = <42>; } }
    let node_b = NodeBuilder::new()
        .node("some_node", NodeBuilder::new().property("prop_2", 42_u32))
        .build();
    // / { some_node { prop_1 = <17>; prop_2 = <42>; } }
    node_a.merge(node_b);
    assert!(node_a.properties.is_empty());
    assert_eq!(node_a.nodes.len(), 1);
    let sub_node = &node_a.nodes["some_node"];
    assert_eq!(sub_node.properties.len(), 2);
    assert!(sub_node.nodes.is_empty());
    assert_eq!(sub_node.properties["prop_1"], vec![17_u32.into()]);
    assert_eq!(sub_node.properties["prop_2"], vec![42_u32.into()]);
}
