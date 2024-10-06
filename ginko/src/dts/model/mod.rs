use std::collections::HashMap;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CellValue<'a> {
    Number(u32),
    Reference(&'a Node<'a>),
}

impl From<u32> for CellValue<'_> {
    fn from(value: u32) -> Self {
        CellValue::Number(value)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Value<'a> {
    Bytes(Vec<u8>),
    String(String),
    Cell(Vec<CellValue<'a>>),
    Reference(&'a Node<'a>),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Node<'a> {
    children: Vec<Node<'a>>,
    properties: HashMap<String, Vec<Value<'a>>>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReservedMemory {
    address: u64,
    length: u64,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct File<'a> {
    reserved_memory: ReservedMemory,
    root: Node<'a>,
}
