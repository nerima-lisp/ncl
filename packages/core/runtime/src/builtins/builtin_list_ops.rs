use super::{arity, exact, index_argument, type_error};
use crate::{RuntimeError, Value};

pub fn reverse(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "reverse", 1)?;
    if let Value::Vector(elements) = &arguments[0] {
        let mut items = elements.visible_snapshot();
        items.reverse();
        return Ok(Value::vector(items));
    }
    if let Value::String(value) = &arguments[0] {
        return Ok(Value::string(value.chars().rev().collect::<String>()));
    }
    reverse_list("reverse", &arguments[0])
}

pub fn nreverse(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "nreverse", 1)?;
    if let Value::Vector(elements) = &arguments[0] {
        let mut items = elements.visible_snapshot();
        items.reverse();
        elements.replace_range(0, &items);
        return Ok(arguments[0].clone());
    }
    let cells = arguments[0]
        .list_cells()
        .ok_or_else(|| type_error("nreverse", "proper list", &arguments[0]))?;
    let mut tail = Value::Nil;
    for cell in cells {
        cell.set_cdr(tail);
        tail = Value::Cons(cell);
    }
    Ok(tail)
}

fn reverse_list(function: &str, value: &Value) -> Result<Value, RuntimeError> {
    let Some(mut items) = value.list_items() else {
        return Err(type_error(function, "list", value));
    };
    items.reverse();
    Ok(Value::list(items))
}

pub fn last(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("last", "one or two", arguments.len()));
    }
    let Some((items, _)) = arguments[0].list_parts() else {
        return Err(type_error("last", "list", &arguments[0]));
    };
    let count = arguments
        .get(1)
        .map(|value| index_argument("last", value))
        .transpose()?
        .unwrap_or(1);
    let start = items.len().saturating_sub(count);
    arguments[0]
        .nth_tail(start)
        .ok_or_else(|| type_error("last", "list", &arguments[0]))
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
    let Some((items, tail)) = arguments[0].list_parts() else {
        return Err(type_error("copy-list", "list", &arguments[0]));
    };
    Ok(Value::dotted_list(items, tail))
}

pub fn copy_alist(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "copy-alist", 1)?;
    let Some(entries) = arguments[0].list_items() else {
        return Err(type_error("copy-alist", "association list", &arguments[0]));
    };
    let copied = entries
        .into_iter()
        .map(|entry| match entry {
            Value::Cons(cell) => Ok(Value::cons(cell.car(), cell.cdr())),
            Value::Nil | Value::Boolean(false) => Ok(Value::Nil),
            value => Err(type_error("copy-alist", "association", &value)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::list(copied))
}

pub fn copy_tree(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "copy-tree", 1)?;
    Ok(copy_tree_value(&arguments[0]))
}

pub fn rplaca(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rplaca", 2)?;
    let Value::Cons(cell) = &arguments[0] else {
        return Err(type_error("rplaca", "a cons", &arguments[0]));
    };
    cell.set_car(arguments[1].clone());
    Ok(arguments[0].clone())
}

pub fn rplacd(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rplacd", 2)?;
    let Value::Cons(cell) = &arguments[0] else {
        return Err(type_error("rplacd", "a cons", &arguments[0]));
    };
    cell.set_cdr(arguments[1].clone());
    Ok(arguments[0].clone())
}

pub fn tailp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "tailp", 2)?;
    let mut current = arguments[1].clone();
    loop {
        if current.eq_value(&arguments[0]) {
            return Ok(Value::boolean(true));
        }
        current = match current {
            Value::Cons(cell) => cell.cdr(),
            Value::Nil | Value::Boolean(false) => return Ok(Value::Nil),
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
            Value::Cons(cell) => {
                prefix.push(cell.car());
                cell.cdr()
            }
            Value::Nil | Value::Boolean(false) => {
                return Err(type_error("ldiff", "tail of list", target));
            }
            value => {
                let result = Value::list(prefix);
                let mut current = result.clone();
                while let Value::Cons(cell) = current {
                    if matches!(cell.cdr(), Value::Nil | Value::Boolean(false)) {
                        cell.set_cdr(value.clone());
                        return Ok(result);
                    }
                    current = cell.cdr();
                }
                return Ok(result);
            }
        };
    }
}

fn copy_tree_value(value: &Value) -> Value {
    fn copy(value: &Value, active: &mut std::collections::HashMap<usize, Value>) -> Value {
        let Value::Cons(cell) = value else {
            return value.clone();
        };
        if let Some(existing) = active.get(&cell.identity()) {
            return existing.clone();
        }
        let result = Value::cons(Value::Nil, Value::Nil);
        active.insert(cell.identity(), result.clone());
        if let Value::Cons(target) = &result {
            target.set_car(copy(&cell.car(), active));
            target.set_cdr(copy(&cell.cdr(), active));
        }
        active.remove(&cell.identity());
        result
    }
    copy(value, &mut std::collections::HashMap::new())
}
