use crate::dts::model::{CellValue, CellValues, NodeName, Path, Reference, Value};
use std::fmt::{Display, Formatter, UpperHex};

impl<T> Display for CellValue<T>
where
    T: UpperHex,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CellValue::Number(number) => write!(f, "0x{number:X}"),
            CellValue::Reference(reference) => write!(f, "{reference}"),
        }
    }
}

impl Display for Reference {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Reference::Path(path) => write!(f, "&{{{path}}}"),
            Reference::Label(label) => write!(f, "&{label}"),
        }
    }
}

impl Display for Path {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for component in &self.components {
            write!(f, "/")?;
            write!(f, "{component}")?;
        }
        Ok(())
    }
}

impl Display for NodeName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.ident)?;
        if let Some(address) = &self.address {
            write!(f, "@{address}")?;
        }
        Ok(())
    }
}

#[test]
fn check_cell_value_formatted() {
    assert_eq!(format!("{}", CellValue::Number(10_u8)), "0xA");
    assert_eq!(format!("{}", CellValue::Number(42_u16)), "0x2A")
}

fn join_formatted<T>(values: &[T], separator: &str, f: &mut Formatter<'_>) -> std::fmt::Result
where
    T: Display,
{
    for (i, value) in values.iter().enumerate() {
        write!(f, "{value}")?;
        if i != values.len() - 1 {
            write!(f, "{separator}")?
        }
    }
    Ok(())
}

impl Display for CellValues {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CellValues::U8(values) => join_formatted(values, " ", f),
            CellValues::U16(values) => join_formatted(values, " ", f),
            CellValues::U32(values) => join_formatted(values, " ", f),
            CellValues::U64(values) => join_formatted(values, " ", f),
        }
    }
}

#[test]
fn check_cell_values_formatted() {
    assert_eq!(
        format!("{}", CellValues::U8(vec![CellValue::Number(10_u8)])),
        "0xA"
    );
    assert_eq!(
        format!(
            "{}",
            CellValues::U8(vec![CellValue::Number(10_u8), CellValue::Number(20_u8)])
        ),
        "0xA 0x14"
    );
}

impl Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Bytes(bytes) => {
                write!(f, "[")?;
                for (i, byte) in bytes.iter().enumerate() {
                    write!(f, "{byte:02X}")?;
                    if i != bytes.len() - 1 {
                        write!(f, " ")?;
                    }
                }
                write!(f, "]")
            }
            Value::String(string) => {
                write!(
                    f,
                    "\"{}\"",
                    string.chars().fold(String::new(), |mut buf, ch| {
                        if ch == '"' || ch == '\\' {
                            buf.push('\\');
                        }
                        buf.push(ch);
                        buf
                    })
                )
            }
            Value::Cell(cell) => {
                write!(f, "<")?;
                write!(f, "{cell}")?;
                write!(f, ">")
            }
            Value::Reference(reference) => write!(f, "{reference}"),
        }
    }
}

#[test]
fn check_values_formatted() {
    assert_eq!(format!("{}", Value::from(3_u32)), "<0x3>");
    assert_eq!(format!("{}", Value::from([0xAB, 0xCD, 0xEF])), "[AB CD EF]");
    assert_eq!(
        format!("{}", Value::from("Hello, World!")),
        r#""Hello, World!""#
    );
    assert_eq!(
        format!("{}", Value::from("Hello, \\\"World!\"")),
        r#""Hello, \\\"World!\"""#
    );
}
