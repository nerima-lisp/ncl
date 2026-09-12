use std::cell::Cell;
use std::rc::Rc;

use super::{Environment, Form, FormKind, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_with_hash_table_iterator(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() < 2 {
            return Err(Self::arity(
                "with-hash-table-iterator",
                "at least one",
                items.len().saturating_sub(1),
            ));
        }
        let FormKind::List(binding) = &items[1].kind else {
            return Err(Self::invalid(
                "hash-table iterator binding must be a list",
                items[1].span,
            ));
        };
        if binding.len() != 2 {
            return Err(Self::invalid(
                "hash-table iterator binding needs a name and hash-table",
                items[1].span,
            ));
        }
        let (name, escaped) =
            Self::variable_name_info(&binding[0], "hash-table iterator name must be a symbol")?;
        let table = self
            .eval_values_in(&binding[1], environment)?
            .primary_value();
        let Some(entries) = table.hash_table_entries() else {
            return Err(RuntimeError::Type {
                expected: "with-hash-table-iterator requires hash-table".to_owned(),
                actual: table.type_name().to_owned(),
                span: Some(binding[1].span),
            });
        };
        let function = Value::Function(Rc::new(crate::Function::HashTableIterator {
            entries: Rc::new(entries.borrow().clone()),
            index: Rc::new(Cell::new(0)),
        }));
        let local = environment.child();
        if escaped {
            local.define_function_exact(name, function);
        } else {
            local.define_function(name, function);
        }
        self.eval_sequence_values(&items[2..], &local)
    }
}
