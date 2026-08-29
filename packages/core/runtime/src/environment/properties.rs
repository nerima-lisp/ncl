use crate::Value;
use crate::environment::Environment;

impl Environment {
    pub(crate) fn symbol_plist(&self, symbol: &Value) -> Option<Value> {
        let (plist, parent) = {
            let frame = self.0.borrow();
            (
                frame
                    .symbol_properties
                    .iter()
                    .find(|(stored_symbol, _)| stored_symbol.eq_value(symbol))
                    .map(|(_, plist)| plist.clone()),
                frame.parent.clone(),
            )
        };
        plist.or_else(|| parent.and_then(|environment| environment.symbol_plist(symbol)))
    }

    pub(crate) fn set_symbol_plist(&self, symbol: &Value, plist: Value) {
        let parent = {
            let mut frame = self.0.borrow_mut();
            if let Some((_, stored_plist)) = frame
                .symbol_properties
                .iter_mut()
                .find(|(stored_symbol, _)| stored_symbol.eq_value(symbol))
            {
                *stored_plist = plist;
                return;
            }
            frame.parent.clone()
        };
        if let Some(parent) = parent {
            parent.set_symbol_plist(symbol, plist);
        } else {
            self.0
                .borrow_mut()
                .symbol_properties
                .push((symbol.clone(), plist));
        }
    }

    pub(crate) fn remove_symbol_property(&self, symbol: &Value) -> Option<Value> {
        let (removed, parent) = {
            let mut frame = self.0.borrow_mut();
            let index = frame
                .symbol_properties
                .iter()
                .position(|(stored_symbol, _)| stored_symbol.eq_value(symbol));
            (
                index.map(|index| frame.symbol_properties.remove(index).1),
                frame.parent.clone(),
            )
        };
        removed
            .or_else(|| parent.and_then(|environment| environment.remove_symbol_property(symbol)))
    }
}

#[cfg(test)]
mod tests {
    use crate::Value;
    use crate::environment::Environment;

    fn assert_integer(value: Option<&Value>, expected: i64) {
        assert!(matches!(value, Some(Value::Integer(actual)) if *actual == expected));
    }

    #[test]
    fn symbol_property_bindings_update_remove_and_compare_symbols() {
        let root = Environment::new();
        let child = root.child();
        let symbol = Value::symbol("name");

        assert!(root.symbol_plist(&symbol).is_none());
        root.set_symbol_plist(&symbol, Value::Integer(1));
        assert_integer(child.symbol_plist(&Value::symbol("NAME")).as_ref(), 1);
        child.set_symbol_plist(&symbol, Value::Integer(2));
        assert_integer(root.symbol_plist(&symbol).as_ref(), 2);
        assert_integer(child.remove_symbol_property(&symbol).as_ref(), 2);
        assert!(root.symbol_plist(&symbol).is_none());
    }
}
