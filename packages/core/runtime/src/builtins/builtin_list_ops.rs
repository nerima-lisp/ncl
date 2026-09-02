use super::{arity, exact, index_argument, type_error};
use crate::{RuntimeError, Value};

pub fn reverse(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "reverse", 1)?;
    reverse_list("reverse", &arguments[0])
}

pub fn nreverse(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "nreverse", 1)?;
    reverse_list("nreverse", &arguments[0])
}

fn reverse_list(function: &str, value: &Value) -> Result<Value, RuntimeError> {
    match value {
        Value::Nil => Ok(Value::Nil),
        Value::List(items) => {
            let mut items = items.as_ref().clone();
            items.reverse();
            Ok(Value::list(items))
        }
        Value::MutableCons(_) => {
            let mut items = value
                .list_items()
                .ok_or_else(|| type_error(function, "sequence", value))?;
            items.reverse();
            Ok(Value::list(items))
        }
        Value::Vector(items) => {
            let mut items = items.borrow().clone();
            items.reverse();
            Ok(Value::vector(items))
        }
        Value::String(value) => Ok(Value::string(value.chars().rev().collect::<String>())),
        _ => Err(type_error(function, "sequence", value)),
    }
}

pub fn last(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("last", "one or two", arguments.len()));
    }
    let Some(items) = arguments[0].list_items() else {
        return Err(type_error("last", "list", &arguments[0]));
    };
    let count = arguments
        .get(1)
        .map(|value| index_argument("last", value))
        .transpose()?
        .unwrap_or(1);
    if count == 0 {
        return Ok(Value::Nil);
    }
    let start = items.len().saturating_sub(count);
    Ok(Value::list(items[start..].to_vec()))
}

pub fn butlast(arguments: &[Value]) -> Result<Value, RuntimeError> {
    butlast_like("butlast", arguments)
}

pub fn nbutlast(arguments: &[Value]) -> Result<Value, RuntimeError> {
    butlast_like("nbutlast", arguments)
}

fn butlast_like(function: &str, arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity(function, "one or two", arguments.len()));
    }
    let Some(items) = arguments[0].list_items() else {
        return Err(type_error(function, "list", &arguments[0]));
    };
    let count = arguments
        .get(1)
        .map(|value| index_argument(function, value))
        .transpose()?
        .unwrap_or(1);
    let end = items.len().saturating_sub(count);
    Ok(Value::list(items[..end].to_vec()))
}

pub fn copy_list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "copy-list", 1)?;
    let Some(items) = arguments[0].list_items() else {
        return Err(type_error("copy-list", "list", &arguments[0]));
    };
    Ok(Value::list(items))
}

pub fn copy_alist(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "copy-alist", 1)?;
    let Some(entries) = arguments[0].list_items() else {
        return Err(type_error("copy-alist", "association list", &arguments[0]));
    };
    let copied = entries
        .into_iter()
        .map(|entry| match entry {
            Value::List(items) => Ok(Value::list(items.as_ref().clone())),
            Value::DottedList { items, tail } => Ok(Value::dotted_list(
                items.as_ref().clone(),
                tail.as_ref().clone(),
            )),
            value => Err(type_error("copy-alist", "association", &value)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::list(copied))
}

pub fn copy_tree(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "copy-tree", 1)?;
    Ok(copy_tree_value(&arguments[0]))
}

pub fn tailp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "tailp", 2)?;
    let mut current = arguments[1].clone();
    loop {
        if current.eq_value(&arguments[0]) {
            return Ok(Value::Boolean(true));
        }
        current = match current {
            Value::MutableCons(cell) => cell.borrow().1.clone(),
            Value::Nil | Value::List(_) => return Ok(Value::Nil),
            Value::DottedList { tail, .. } => {
                return Ok(Value::boolean(tail.eq_value(&arguments[0])));
            }
            value => return Err(type_error("tailp", "list", &value)),
        };
    }
}

pub fn ldiff(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "ldiff", 2)?;
    let target = &arguments[1];
    let mut current = arguments[0].clone();
    let mut prefix = Vec::new();
    loop {
        if current.eq_value(target) {
            return Ok(Value::list(prefix));
        }
        current = match current {
            Value::MutableCons(cell) => {
                let cell = cell.borrow();
                prefix.push(cell.0.clone());
                cell.1.clone()
            }
            Value::Nil | Value::List(_) => {
                return Err(type_error("ldiff", "tail of list", target));
            }
            Value::DottedList { items, tail } => {
                prefix.extend(items.iter().cloned());
                tail.as_ref().clone()
            }
            value => return Err(type_error("ldiff", "list", &value)),
        };
    }
}

pub fn subst(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "subst", 3)?;
    Ok(subst_tree(&arguments[2], &arguments[1], &arguments[0]))
}

pub fn nsubst(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "nsubst", 3)?;
    Ok(nsubst_tree(&arguments[2], &arguments[1], &arguments[0]))
}

pub fn sublis(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "sublis", 2)?;
    let substitutions = alist_entries("sublis", &arguments[0])?;
    Ok(sublis_tree(&arguments[1], &substitutions))
}

pub fn nsublis(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "nsublis", 2)?;
    let substitutions = alist_entries("nsublis", &arguments[0])?;
    Ok(nsublis_tree(&arguments[1], &substitutions))
}

pub fn tree_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "tree-equal", 2)?;
    Ok(Value::boolean(arguments[0].equal_value(&arguments[1])))
}

fn subst_tree(value: &Value, old: &Value, new: &Value) -> Value {
    if value.eq_value(old) {
        return new.clone();
    }
    match value {
        Value::List(items) => Value::list(
            items
                .iter()
                .map(|item| subst_tree(item, old, new))
                .collect(),
        ),
        Value::MutableCons(cell) => {
            let (car, cdr) = {
                let cell = cell.borrow();
                (cell.0.clone(), cell.1.clone())
            };
            Value::cons_cell(subst_tree(&car, old, new), subst_tree(&cdr, old, new))
        }
        Value::DottedList { items, tail } => Value::dotted_list(
            items
                .iter()
                .map(|item| subst_tree(item, old, new))
                .collect(),
            subst_tree(tail, old, new),
        ),
        value => value.clone(),
    }
}

fn nsubst_tree(value: &Value, old: &Value, new: &Value) -> Value {
    if value.eq_value(old) {
        return new.clone();
    }
    match value {
        Value::MutableCons(cell) => {
            let (car, cdr) = {
                let cell = cell.borrow();
                (cell.0.clone(), cell.1.clone())
            };
            let car = nsubst_tree(&car, old, new);
            let cdr = nsubst_tree(&cdr, old, new);
            let mut cell = cell.borrow_mut();
            cell.0 = car;
            cell.1 = cdr;
            value.clone()
        }
        Value::List(items) => Value::list(
            items
                .iter()
                .map(|item| nsubst_tree(item, old, new))
                .collect(),
        ),
        Value::DottedList { items, tail } => Value::dotted_list(
            items
                .iter()
                .map(|item| nsubst_tree(item, old, new))
                .collect(),
            nsubst_tree(tail, old, new),
        ),
        value => value.clone(),
    }
}

fn alist_entries(function: &str, value: &Value) -> Result<Vec<(Value, Value)>, RuntimeError> {
    let Some(entries) = value.list_items() else {
        return Err(type_error(function, "association list", value));
    };
    entries
        .into_iter()
        .map(|entry| match entry {
            Value::DottedList { items, tail } if items.len() == 1 => {
                Ok((items[0].clone(), tail.as_ref().clone()))
            }
            Value::List(items) if items.len() == 2 => Ok((items[0].clone(), items[1].clone())),
            value => Err(type_error(function, "association", &value)),
        })
        .collect()
}

fn substitution(value: &Value, substitutions: &[(Value, Value)]) -> Option<Value> {
    substitutions
        .iter()
        .find(|(old, _)| value.eq_value(old))
        .map(|(_, new)| new.clone())
}

fn sublis_tree(value: &Value, substitutions: &[(Value, Value)]) -> Value {
    if let Some(new) = substitution(value, substitutions) {
        return new;
    }
    match value {
        Value::List(items) => Value::list(
            items
                .iter()
                .map(|item| sublis_tree(item, substitutions))
                .collect(),
        ),
        Value::MutableCons(cell) => {
            let (car, cdr) = {
                let cell = cell.borrow();
                (cell.0.clone(), cell.1.clone())
            };
            Value::cons_cell(
                sublis_tree(&car, substitutions),
                sublis_tree(&cdr, substitutions),
            )
        }
        Value::DottedList { items, tail } => Value::dotted_list(
            items
                .iter()
                .map(|item| sublis_tree(item, substitutions))
                .collect(),
            sublis_tree(tail, substitutions),
        ),
        value => value.clone(),
    }
}

fn nsublis_tree(value: &Value, substitutions: &[(Value, Value)]) -> Value {
    if let Some(new) = substitution(value, substitutions) {
        return new;
    }
    match value {
        Value::MutableCons(cell) => {
            let (car, cdr) = {
                let cell = cell.borrow();
                (cell.0.clone(), cell.1.clone())
            };
            let car = nsublis_tree(&car, substitutions);
            let cdr = nsublis_tree(&cdr, substitutions);
            let mut cell = cell.borrow_mut();
            cell.0 = car;
            cell.1 = cdr;
            value.clone()
        }
        Value::List(items) => Value::list(
            items
                .iter()
                .map(|item| nsublis_tree(item, substitutions))
                .collect(),
        ),
        Value::DottedList { items, tail } => Value::dotted_list(
            items
                .iter()
                .map(|item| nsublis_tree(item, substitutions))
                .collect(),
            nsublis_tree(tail, substitutions),
        ),
        value => value.clone(),
    }
}

fn copy_tree_value(value: &Value) -> Value {
    match value {
        Value::List(items) => Value::list(items.iter().map(copy_tree_value).collect()),
        Value::MutableCons(cell) => {
            let (car, cdr) = {
                let cell = cell.borrow();
                (cell.0.clone(), cell.1.clone())
            };
            Value::cons_cell(copy_tree_value(&car), copy_tree_value(&cdr))
        }
        Value::DottedList { items, tail } => Value::dotted_list(
            items.iter().map(copy_tree_value).collect(),
            copy_tree_value(tail),
        ),
        _ => value.clone(),
    }
}
