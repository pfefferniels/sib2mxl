//! Field access that ignores how the format grew: fields that later versions
//! appended inside records are found as if they sat in the object itself, and
//! part maps yield the value of the full score.

use libsib::tree::{Node, Value};

pub trait Fields {
    fn value(&self, name: &str) -> Option<&Value>;

    fn number(&self, name: &str) -> Option<i64> {
        self.value(name)?.as_int()
    }

    /// A number Sibelius stores unsigned but means as signed, such as an offset.
    fn signed(&self, name: &str) -> Option<i32> {
        match *self.value(name)? {
            Value::U16(n) => Some(i32::from(n as i16)),
            Value::U32(n) => Some(n as i32),
            ref other => other.as_int().map(|n| n as i32),
        }
    }

    fn float(&self, name: &str) -> Option<f64> {
        match *self.value(name)? {
            Value::F32(bits) => Some(f64::from(f32::from_bits(bits))),
            ref other => other.as_int().map(|n| n as f64),
        }
    }

    fn flag(&self, name: &str) -> bool {
        self.number(name).is_some_and(|n| n != 0)
    }

    fn string(&self, name: &str) -> Option<String> {
        string(self.value(name)?)
    }

    fn child(&self, name: &str) -> Option<&Node> {
        node(self.value(name)?)
    }

    fn children(&self, name: &str) -> Vec<&Node> {
        match self.value(name) {
            Some(Value::List(list)) => list.items.iter().filter_map(node).collect(),
            _ => Vec::new(),
        }
    }

    /// A list of plain numbers.
    fn numbers(&self, name: &str) -> Vec<i64> {
        match self.value(name) {
            Some(Value::List(list)) => list.items.iter().filter_map(Value::as_int).collect(),
            _ => Vec::new(),
        }
    }

    /// The first string below `name`: the id inside a style reference, however
    /// it is wrapped.
    fn style(&self, name: &str) -> Option<String> {
        first_string(self.value(name)?)
    }

    /// The first string field called `name` at any depth.
    fn nested_string(&self, name: &str) -> Option<String>;
}

impl Fields for Node {
    fn value(&self, name: &str) -> Option<&Value> {
        self.fields.iter().find_map(|(field, value)| match value {
            _ if *field == name => Some(score_value(value)),
            Value::Record(record) => record.body.value(name),
            _ => None,
        })
    }

    fn nested_string(&self, name: &str) -> Option<String> {
        self.fields.iter().find_map(|(field, value)| {
            let value = score_value(value);
            match string(value) {
                Some(text) if *field == name => Some(text),
                _ => node(value)?.nested_string(name),
            }
        })
    }
}

fn score_value(value: &Value) -> &Value {
    match value {
        Value::Node(map) if map.kind == "PartMap" => map.get("default").unwrap_or(value),
        _ => value,
    }
}

fn node(value: &Value) -> Option<&Node> {
    match value {
        Value::Node(node) => Some(node),
        Value::Record(record) => Some(&record.body),
        _ => None,
    }
}

fn string(value: &Value) -> Option<String> {
    match value {
        Value::Str(units) => Some(String::from_utf16_lossy(units)),
        Value::Str8(bytes) => Some(bytes.iter().map(|&b| char::from(b)).collect()),
        _ => None,
    }
}

fn first_string(value: &Value) -> Option<String> {
    string(value).or_else(|| {
        node(value)?
            .fields
            .iter()
            .find_map(|(_, value)| first_string(score_value(value)))
    })
}
