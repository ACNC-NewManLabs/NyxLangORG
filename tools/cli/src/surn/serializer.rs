use crate::surn::ast::*;

pub struct Serializer;

impl Serializer {
    pub fn serialize(doc: &Document) -> String {
        let mut output = String::new();
        for stmt in &doc.statements {
            output.push_str(&Self::serialize_statement(stmt, 0));
        }
        output
    }

    fn serialize_statement(stmt: &Statement, indent: usize) -> String {
        let mut s = String::new();
        let indent_str = "    ".repeat(indent);
        match stmt {
            Statement::Assignment { key, value } => {
                s.push_str(&format!("{}{} = {}\n", indent_str, key, Self::serialize_value(value)));
            }
            Statement::Table { header, assignments, is_double } => {
                let brackets = if *is_double { "[[" } else { "[" };
                let end_brackets = if *is_double { "]]" } else { "]" };
                s.push_str(&format!("\n{}{} {}\n", brackets, header.join("."), end_brackets));
                for a in assignments {
                    s.push_str(&Self::serialize_statement(a, 0));
                }
            }
            Statement::Block { key, children } => {
                s.push_str(&format!("{}{}:\n", indent_str, key));
                for child in children {
                    s.push_str(&Self::serialize_statement(child, indent + 1));
                }
            }
        }
        s
    }

    fn serialize_value(val: &Value) -> String {
        match val {
            Value::String(s) => format!("\"{}\"", s), // Simplified escaping
            Value::Integer(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Boolean(b) => b.to_string(),
            Value::Null => "null".to_string(),
            Value::DateTime(dt) => dt.to_string(),
            Value::Binary(b) => format!("hex({})", hex::encode(b)),
            Value::Array(arr) => {
                let inner: Vec<String> = arr.iter().map(Self::serialize_value).collect();
                format!("[{}]", inner.join(", "))
            }
            Value::Object(obj) => {
                let inner: Vec<String> = obj.iter()
                    .map(|(k, v)| format!("{}: {}", k, Self::serialize_value(v)))
                    .collect();
                format!("{{ {} }}", inner.join(", "))
            }
        }
    }
}
