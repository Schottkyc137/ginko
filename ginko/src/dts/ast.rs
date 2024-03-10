use crate::dts::data::HasSource;
use crate::dts::lexer::Token;
use crate::dts::{HasSpan, Span};
use itertools::Itertools;
use std::fmt::{Display, Formatter, LowerHex};
use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct WithToken<T> {
    item: T,
    token: Token,
}

impl<T> HasSpan for WithToken<T> {
    fn span(&self) -> Span {
        self.token.span
    }
}

impl<T> HasSource for WithToken<T> {
    fn source(&self) -> Arc<str> {
        self.token.source()
    }
}

impl<T> WithToken<T> {
    pub fn new(item: T, token: Token) -> WithToken<T> {
        WithToken { item, token }
    }

    pub fn item(&self) -> &T {
        &self.item
    }
}

impl<T> Deref for WithToken<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl<T> Display for WithToken<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.item())
    }
}

impl<T> LowerHex for WithToken<T>
where
    T: LowerHex,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:x}", self.item())
    }
}

// LRM 2.2.1 – Node Names
#[derive(Eq, PartialEq, Debug, Hash, Clone)]
pub struct NodeName {
    pub name: String,
    pub unit_address: Option<String>,
}

impl From<String> for NodeName {
    fn from(value: String) -> Self {
        if let Some((prefix, suffix)) = value.split_once('@') {
            NodeName::with_address(prefix, suffix)
        } else {
            NodeName::simple(value)
        }
    }
}

impl From<&str> for NodeName {
    fn from(value: &str) -> Self {
        if let Some((prefix, suffix)) = value.split_once('@') {
            NodeName::with_address(prefix, suffix)
        } else {
            NodeName::simple(value)
        }
    }
}

impl From<WithToken<String>> for WithToken<NodeName> {
    fn from(value: WithToken<String>) -> Self {
        WithToken::new(NodeName::from(value.item), value.token)
    }
}

impl NodeName {
    pub fn simple(name: impl Into<String>) -> NodeName {
        NodeName {
            name: name.into(),
            unit_address: None,
        }
    }

    pub fn with_address(name: impl Into<String>, address: impl Into<String>) -> NodeName {
        NodeName {
            name: name.into(),
            unit_address: Some(address.into()),
        }
    }
}

impl Display for NodeName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(unit_address) = &self.unit_address {
            write!(f, "@{}", unit_address)?;
        }
        Ok(())
    }
}

// LRM 2.2.3 – Paths
#[derive(Eq, PartialEq, Debug, Hash, Clone)]
pub struct Path {
    elements: Vec<NodeName>,
}

impl Display for Path {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.elements.is_empty() {
            write!(f, "/")
        } else {
            for element in &self.elements {
                write!(f, "/{}", element)?;
            }
            Ok(())
        }
    }
}

impl Path {
    pub fn new(elements: Vec<NodeName>) -> Path {
        Path { elements }
    }

    pub fn empty() -> Path {
        Path { elements: vec![] }
    }

    pub fn with_child(&self, child: NodeName) -> Path {
        let mut new_elements = self.elements.clone();
        new_elements.push(child);
        Path {
            elements: new_elements,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &NodeName> {
        self.elements.iter()
    }
}

impl From<&str> for Path {
    fn from(value: &str) -> Self {
        Path::new(
            value
                .split('/')
                .filter(|component| !component.is_empty())
                .map(NodeName::from)
                .collect_vec(),
        )
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum Reference {
    Label(String),
    Path(Path),
}

impl Display for Reference {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Reference::Label(label) => write!(f, "&{label}"),
            Reference::Path(path) => write!(f, "&{{{path}}}"),
        }
    }
}

#[derive(Eq, PartialEq, Debug)]
pub enum Cell {
    Number(WithToken<u32>),
    Reference(WithToken<Reference>),
    Expression,
}

impl Display for Cell {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Cell::Number(num) => write!(f, "0x{num:x}"),
            Cell::Reference(reference) => write!(f, "{reference}"),
            Cell::Expression => write!(f, "(not implemented)"),
        }
    }
}

// LRM 2.2.4 Property Values
#[derive(Eq, PartialEq, Debug)]
pub enum PropertyValue {
    String(WithToken<String>),
    Cells(Token, Vec<Cell>, Token),
    Reference(WithToken<Reference>),
    ByteStrings(Token, Vec<WithToken<Vec<u8>>>, Token),
}

impl HasSpan for PropertyValue {
    fn span(&self) -> Span {
        match self {
            PropertyValue::String(str) => str.span(),
            PropertyValue::Cells(start, _, end) => start.start().to(end.end()),
            PropertyValue::Reference(reference) => reference.span(),
            PropertyValue::ByteStrings(start, _, end) => start.start().to(end.end()),
        }
    }
}

impl PropertyValue {
    pub fn source(&self) -> Arc<str> {
        match self {
            PropertyValue::String(str) => str.token.source(),
            PropertyValue::Cells(start, ..) => start.source.clone(),
            PropertyValue::Reference(reference) => reference.token.source.clone(),
            PropertyValue::ByteStrings(start, ..) => start.source.clone(),
        }
    }
}

impl Display for PropertyValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            PropertyValue::String(string) => {
                write!(f, "\"{string}\"")
            }
            PropertyValue::Cells(_, numbers, _) => {
                write!(f, "<")?;
                for (i, num) in numbers.iter().enumerate() {
                    write!(f, "{num}")?;
                    if i != numbers.len() - 1 {
                        write!(f, " ")?;
                    }
                }
                write!(f, ">")
            }
            PropertyValue::Reference(reference) => write!(f, "{reference}",),
            PropertyValue::ByteStrings(_, strings, _) => {
                write!(f, "[")?;
                for (i, numbers) in strings.iter().enumerate() {
                    for num in &numbers.item {
                        write!(f, "{num:2x}")?;
                    }
                    if i != strings.len() - 1 {
                        write!(f, " ")?;
                    }
                }
                write!(f, "]")
            }
        }
    }
}

// LRM 2.2.4 Property Values
#[derive(Eq, PartialEq, Debug)]
pub struct Property {
    pub label: Option<WithToken<String>>,
    pub name: WithToken<String>,
    pub values: Vec<PropertyValue>,
    pub end: Token,
}

impl HasSpan for Property {
    fn span(&self) -> Span {
        self.label
            .as_ref()
            .map(|label| label.token.span())
            .unwrap_or(self.name.span())
            .start()
            .to(self.end.end())
    }
}

impl Property {
    pub fn empty(
        name: WithToken<String>,
        label: Option<WithToken<String>>,
        end: Token,
    ) -> Property {
        Property {
            label,
            name,
            values: vec![],
            end,
        }
    }

    #[cfg(test)]
    pub fn simple(
        name: WithToken<String>,
        value: PropertyValue,
        label: Option<WithToken<String>>,
        end: Token,
    ) -> Property {
        Property {
            label,
            name,
            values: vec![value],
            end,
        }
    }
}

impl Display for Property {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.values.is_empty() {
            writeln!(f, "{};", self.name)
        } else {
            write!(f, "{} = ", self.name)?;
            for (i, value) in self.values.iter().enumerate() {
                write!(f, "{value}")?;
                if i != self.values.len() - 1 {
                    write!(f, ", ")?;
                }
            }
            writeln!(f, ";")
        }
    }
}

#[derive(Eq, PartialEq, Debug)]
pub struct Node {
    pub label: Option<WithToken<String>>,
    pub name: WithToken<NodeName>,
    pub payload: NodePayload,
}

#[derive(Eq, PartialEq, Debug)]
pub struct NodePayload {
    pub properties: Vec<Arc<Property>>,
    pub child_nodes: Vec<Arc<Node>>,
    pub end: Token,
}

impl Display for NodePayload {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{{")?;
        for prop in &self.properties {
            writeln!(f, "    {prop}")?;
        }
        for child_node in &self.child_nodes {
            writeln!(f, "    {child_node}")?;
        }
        write!(f, "}};")
    }
}

impl HasSpan for Node {
    fn span(&self) -> Span {
        self.label
            .as_ref()
            .map(|lbl| lbl.span())
            .unwrap_or(self.name.span())
            .start()
            .to(self.payload.end.span.end())
    }
}

impl Display for Node {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(label) = &self.label {
            write!(f, "{}: ", label.item)?;
        }
        write!(f, "{} {}", self.name.item(), self.payload)
    }
}

#[derive(Eq, PartialEq, Debug)]
pub struct Memreserve {
    address: WithToken<u64>,
    length: WithToken<u64>,
}

impl Memreserve {
    pub fn new(address: WithToken<u64>, length: WithToken<u64>) -> Memreserve {
        Memreserve { address, length }
    }
}

impl Display for Memreserve {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "/memreserve/ 0x{:x} 0x{:x};",
            *self.address, *self.length
        )
    }
}

#[derive(Eq, PartialEq, Debug)]
pub struct DtsFile {
    pub elements: Vec<Primary>,
    pub source: Arc<str>,
}

impl HasSource for DtsFile {
    fn source(&self) -> Arc<str> {
        self.source.clone()
    }
}

impl Display for DtsFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for primary in &self.elements {
            writeln!(f, "{primary}")?;
        }
        Ok(())
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
#[allow(unused)]
pub enum CompilerDirective {
    DTSVersionHeader,
    MemReserve,
    DeleteNode,
    DeleteProperty,
    Plugin,
    Bits,
    OmitIfNoRef,
    Include,
    Other(String),
}

impl Display for CompilerDirective {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CompilerDirective::DTSVersionHeader => write!(f, "/dts-v1/"),
            CompilerDirective::MemReserve => write!(f, "/memreserve/"),
            CompilerDirective::DeleteNode => write!(f, "/delete-node/"),
            CompilerDirective::DeleteProperty => write!(f, "/delete-property/"),
            CompilerDirective::Plugin => write!(f, "/plugin/"),
            CompilerDirective::Bits => write!(f, "/bits/"),
            CompilerDirective::OmitIfNoRef => write!(f, "/omit-if-no-ref/"),
            CompilerDirective::Include => write!(f, "/include/"),
            CompilerDirective::Other(other) => write!(f, "/{other}/"),
        }
    }
}

#[derive(Eq, PartialEq, Debug)]
pub struct Include {
    pub include_token: Token,
    pub file_name: WithToken<String>,
    pub file: Option<DtsFile>,
}

impl HasSpan for Include {
    fn span(&self) -> Span {
        self.include_token.start().to(self.file_name.end())
    }
}

impl Display for Include {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "/include/ \"{}\"", self.file_name)
    }
}

#[derive(Eq, PartialEq, Debug)]
pub enum AnyDirective {
    DtsHeader(Token),
    Plugin(Token),
    Memreserve(Memreserve),
    Include(Include),
}

impl Display for AnyDirective {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AnyDirective::DtsHeader(_) => write!(f, "/dts-v1/;"),
            AnyDirective::Memreserve(memreserve) => write!(f, "{memreserve};"),
            AnyDirective::Include(include) => write!(f, "{include}"),
            AnyDirective::Plugin(_) => write!(f, "/plugin/;"),
        }
    }
}

#[derive(Eq, PartialEq, Debug)]
pub struct ReferencedNode {
    pub reference: WithToken<Reference>,
    pub payload: NodePayload,
}

impl HasSpan for ReferencedNode {
    fn span(&self) -> Span {
        self.reference.start().to(self.payload.end.end())
    }
}

impl Display for ReferencedNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.reference, self.payload)
    }
}

#[derive(Eq, PartialEq, Debug)]
pub enum Primary {
    Directive(AnyDirective),
    Root(Arc<Node>),
    ReferencedNode(ReferencedNode),
    // C-style includes should be put into a separate pass
    CStyleInclude(String),
}

impl Display for Primary {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Primary::Directive(directive) => write!(f, "{directive}"),
            Primary::Root(node) => write!(f, "{node}"),
            Primary::ReferencedNode(node) => write!(f, "{node}"),
            Primary::CStyleInclude(include) => write!(f, "#include {include}"),
        }
    }
}
