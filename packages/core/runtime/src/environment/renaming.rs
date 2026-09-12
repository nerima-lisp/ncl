use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::Value;
use crate::environment::{Environment, intern_name};

impl Environment {
    pub(crate) fn rename_package_prefix(&self, old_name: &str, new_name: &str) {
        let parent = {
            let mut frame = self.0.borrow_mut();
            rename_rc_map(&mut frame.values, old_name, new_name);
            rename_string_map(&mut frame.exact_values, old_name, new_name);
            rename_rc_set(&mut frame.special_names, old_name, new_name);
            rename_string_set(&mut frame.exact_special_names, old_name, new_name);
            rename_rc_map(&mut frame.symbol_macros, old_name, new_name);
            rename_string_map(&mut frame.exact_symbol_macros, old_name, new_name);
            rename_rc_map(&mut frame.functions, old_name, new_name);
            rename_string_map(&mut frame.exact_functions, old_name, new_name);
            rename_rc_map(&mut frame.setf_functions, old_name, new_name);
            rename_rc_map(&mut frame.setf_expanders, old_name, new_name);
            rename_rc_map(&mut frame.structures, old_name, new_name);
            rename_rc_map(&mut frame.classes, old_name, new_name);
            for (symbol, _) in &mut frame.symbol_properties {
                *symbol = rename_symbol(symbol, old_name, new_name);
            }
            frame.parent.clone()
        };
        if let Some(parent) = parent {
            parent.rename_package_prefix(old_name, new_name);
        }
    }
}

pub(crate) fn rename_rc_map<T>(map: &mut HashMap<Rc<str>, T>, old_name: &str, new_name: &str) {
    let entries = std::mem::take(map);
    for (key, value) in entries {
        map.insert(
            intern_name(&rename_qualified_name(&key, old_name, new_name)),
            value,
        );
    }
}

pub(crate) fn rename_string_map<T>(map: &mut HashMap<String, T>, old_name: &str, new_name: &str) {
    let entries = std::mem::take(map);
    for (key, value) in entries {
        map.insert(rename_qualified_name(&key, old_name, new_name), value);
    }
}

pub(crate) fn rename_rc_set(set: &mut HashSet<Rc<str>>, old_name: &str, new_name: &str) {
    let entries = std::mem::take(set);
    for key in entries {
        set.insert(intern_name(&rename_qualified_name(
            &key, old_name, new_name,
        )));
    }
}

pub(crate) fn rename_string_set(set: &mut HashSet<String>, old_name: &str, new_name: &str) {
    let entries = std::mem::take(set);
    for key in entries {
        set.insert(rename_qualified_name(&key, old_name, new_name));
    }
}

pub(crate) fn rename_qualified_name(name: &str, old_name: &str, new_name: &str) -> String {
    let Some((package_name, symbol_name)) = name.split_once("::") else {
        return name.to_string();
    };
    if package_name.eq_ignore_ascii_case(old_name) {
        format!("{new_name}::{symbol_name}")
    } else {
        name.to_string()
    }
}

fn rename_symbol(symbol: &Value, old_name: &str, new_name: &str) -> Value {
    match symbol {
        Value::Symbol(name) => Value::symbol(rename_qualified_name(name, old_name, new_name)),
        Value::SymbolExact(name) => {
            Value::symbol_exact(rename_qualified_name(name, old_name, new_name))
        }
        _ => symbol.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{Environment, VariableResolution};

    #[test]
    fn renames_qualified_bindings_in_every_frame() {
        let root = Environment::new();
        let child = root.child();
        root.define("OLD::VALUE", Value::Integer(1));
        root.define_exact("OLD::EXACT-VALUE", Value::Integer(2));
        root.define_function("OLD::FUNCTION", Value::Integer(3));
        root.define_function_exact("OLD::EXACT-FUNCTION", Value::Integer(4));
        root.define_setf_function("OLD::SETF", Value::Integer(5));
        root.define_setf_expander("OLD::EXPANDER", Value::Integer(6));
        root.declare_special("OLD::SPECIAL");
        root.declare_special_exact("OLD::EXACT-SPECIAL");
        root.set_symbol_plist(&Value::symbol("OLD::PROPERTY"), Value::Integer(7));

        child.rename_package_prefix("old", "NEW");

        assert!(matches!(
            child.lookup("NEW::VALUE"),
            Some(Value::Integer(1))
        ));
        assert!(matches!(
            child.lookup_exact("NEW::EXACT-VALUE"),
            Some(Value::Integer(2))
        ));
        assert!(matches!(
            child.lookup_function("NEW::FUNCTION"),
            Some(Value::Integer(3))
        ));
        assert!(matches!(
            child.lookup_function_exact("NEW::EXACT-FUNCTION"),
            Some(Value::Integer(4))
        ));
        assert!(matches!(
            child.lookup_setf_function("NEW::SETF"),
            Some(Value::Integer(5))
        ));
        assert!(matches!(
            child.lookup_setf_expander("NEW::EXPANDER"),
            Some(Value::Integer(6))
        ));
        assert!(root.has_local_special(&["NEW::SPECIAL".to_string()]));
        assert!(root.has_local_special_exact("NEW::EXACT-SPECIAL"));
        assert!(matches!(
            child.symbol_plist(&Value::symbol("NEW::PROPERTY")),
            Some(Value::Integer(7))
        ));
        assert!(matches!(
            child.resolve(&["NEW::VALUE".to_string()]),
            VariableResolution::Lexical(Some(Value::Integer(1)))
        ));
        assert!(child.lookup("OLD::VALUE").is_none());
        assert!(child.lookup_exact("OLD::EXACT-VALUE").is_none());
        assert!(child.lookup_function("OLD::FUNCTION").is_none());
        assert!(child.lookup_function_exact("OLD::EXACT-FUNCTION").is_none());
        assert!(child.lookup_setf_function("OLD::SETF").is_none());
        assert!(child.lookup_setf_expander("OLD::EXPANDER").is_none());
        assert!(!root.has_local_special(&["OLD::SPECIAL".to_string()]));
        assert!(!root.has_local_special_exact("OLD::EXACT-SPECIAL"));
        assert!(
            child
                .symbol_plist(&Value::symbol("OLD::PROPERTY"))
                .is_none()
        );
    }
}
