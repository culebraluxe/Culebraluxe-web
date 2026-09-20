//! Bounded condition DSL: identifier WS (==|!=) WS literal

use crate::error::{Result, WorkflowError};
use crate::value::Value;

pub fn is_supported_expression(expression: &str) -> bool {
    parse(expression.trim()).is_some()
}

pub fn evaluate_condition(expression: &str, variables: &Value) -> Result<bool> {
    let (name, op, rhs) = parse(expression.trim()).ok_or_else(|| {
        WorkflowError::Expression(format!("Unsupported workflow expression: {:?}", expression))
    })?;
    let present = variables.as_object().map(|m| m.contains_key(name)).unwrap_or(false);
    let lhs = variables.get(name).cloned().unwrap_or(Value::Null);
    let equal = if !present { false } else { json_eq(&lhs, &rhs) };
    match op {
        "==" => Ok(equal),
        "!=" => Ok(!equal),
        _ => Err(WorkflowError::Expression(format!(
            "Unsupported workflow operator in: {:?}",
            expression
        ))),
    }
}

fn parse(expr: &str) -> Option<(&str, &str, Value)> {
    let bytes = expr.as_bytes();
    if bytes.is_empty() || !(bytes[0].is_ascii_alphabetic() || bytes[0] == b'_') {
        return None;
    }
    let mut i = 1;
    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
    }
    let name = &expr[..i];
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let op = if expr[i..].starts_with("==") {
        i += 2;
        "=="
    } else if expr[i..].starts_with("!=") {
        i += 2;
        "!="
    } else {
        return None;
    };
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let lit = &expr[i..];
    let rhs = parse_literal(lit)?;
    Some((name, op, rhs))
}

fn parse_literal(s: &str) -> Option<Value> {
    match s {
        "true" => Some(Value::Bool(true)),
        "false" => Some(Value::Bool(false)),
        "null" => Some(Value::Null),
        _ if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
            || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2) =>
        {
            if s[1..s.len() - 1].contains('\n') {
                return None;
            }
            Some(Value::String(s[1..s.len() - 1].to_string()))
        }
        _ => {
            if s.parse::<i64>().is_ok() || s.parse::<f64>().is_ok() {
                s.parse::<f64>().ok().map(Value::Number)
            } else {
                None
            }
        }
    }
}

fn json_eq(lhs: &Value, rhs: &Value) -> bool {
    match (lhs, rhs) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => a == b,
        _ => false,
    }
}
