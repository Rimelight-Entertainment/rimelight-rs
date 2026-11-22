//! Small helpers between Rust values and the interpreter's dynamic `Value`. Struct fields use the markup's names.

use slint::{Image, Model as _, ModelRc, SharedString, VecModel};
use slint_interpreter::Value;

/// A count or an index as a Slint number.
pub fn number(n: usize) -> Value {
    Value::Number(n as f64)
}

/// A Slint array.
pub fn list(values: Vec<Value>) -> Value {
    Value::Model(ModelRc::new(VecModel::from(values)))
}

/// A Slint struct with these fields.
pub fn record<const N: usize>(fields: [(&str, Value); N]) -> Value {
    Value::Struct(
        fields
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect(),
    )
}

/// A value of the Slint enum `name`.
pub fn enumeration(name: &str, variant: &str) -> Value {
    Value::EnumerationValue(name.to_string(), variant.to_string())
}

/// The variant name of a Rust enum generated from Slint, as the markup spells it (every variant used here is one word).
pub fn variant_of(value: impl std::fmt::Debug) -> String {
    format!("{value:?}").to_lowercase()
}

/// One field of a struct value, or `Void` when there is none.
pub fn field(value: &Value, name: &str) -> Value {
    match value {
        Value::Struct(fields) => fields.get_field(name).cloned().unwrap_or_default(),
        _ => Value::Void,
    }
}

pub fn text(value: Value) -> SharedString {
    SharedString::try_from(value).unwrap_or_default()
}

pub fn flag(value: Value) -> bool {
    bool::try_from(value).unwrap_or_default()
}

pub fn int(value: Value) -> i32 {
    f64::try_from(value).unwrap_or_default() as i32
}

pub fn length(value: Value) -> f32 {
    f64::try_from(value).unwrap_or_default() as f32
}

pub fn image(value: Value) -> Image {
    Image::try_from(value).unwrap_or_default()
}

/// The variant name of an enum value, or "" for anything else.
pub fn variant_name(value: Value) -> String {
    match value {
        Value::EnumerationValue(_, variant) => variant,
        _ => String::new(),
    }
}

/// The rows of an array value.
pub fn rows(value: &Value) -> Vec<Value> {
    match value {
        Value::Model(model) => model.iter().collect(),
        _ => Vec::new(),
    }
}
