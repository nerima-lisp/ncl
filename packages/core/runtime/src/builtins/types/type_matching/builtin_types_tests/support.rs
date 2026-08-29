use crate::Value;

pub(super) fn compound(operator: &str, arguments: Vec<Value>) -> Value {
    let mut items = vec![Value::symbol(operator)];
    items.extend(arguments);
    Value::list(items)
}
