use super::{Environment, Form, FormKind, Runtime, RuntimeError, SetfExpansion, atom_name};
use crate::environment::{intern_name, names_equal};
use crate::package;

impl Runtime {
    pub(crate) fn get_modify_macro_setf_expansion(
        &self,
        place: &Form,
        environment: &Environment,
    ) -> Result<SetfExpansion, RuntimeError> {
        if let Some(expanded) = Self::expand_symbol_macro_form(place, environment)? {
            return self.get_modify_macro_setf_expansion(&expanded, environment);
        }
        if atom_name(place).is_some() {
            return self.get_setf_expansion(place, environment);
        }

        let FormKind::List(items) = &place.kind else {
            return Err(Self::invalid("unsupported SETF place", place.span));
        };
        let Some(operator) = items.first().and_then(atom_name) else {
            return Err(Self::invalid("unsupported SETF place", place.span));
        };
        if let Some(expansion) = self.custom_setf_expansion(place, items, environment)? {
            return Ok(expansion);
        }
        let Some(container_index) =
            Self::modify_macro_container_index(operator, items.len().saturating_sub(1))
        else {
            return self.get_setf_expansion(place, environment);
        };

        let outer_temporaries = items[1..]
            .iter()
            .map(|_| self.fresh_setf_temporary(place.span))
            .collect::<Vec<_>>();
        let outer_values = items[1..].to_vec();
        let nested =
            self.get_modify_macro_setf_expansion(&outer_values[container_index], environment)?;

        let mut temporaries = Vec::new();
        let mut values = Vec::new();
        for (index, (temporary, value_form)) in outer_temporaries
            .iter()
            .zip(outer_values.iter())
            .enumerate()
        {
            if index == container_index {
                temporaries.extend(nested.temporaries.iter().cloned());
                values.extend(nested.values.iter().cloned());
                temporaries.push(temporary.clone());
                values.push(nested.access_form.clone());
            } else {
                temporaries.push(temporary.clone());
                values.push(value_form.clone());
            }
        }

        let mut access_items = Vec::with_capacity(items.len());
        access_items.push(items[0].clone());
        access_items.extend(outer_temporaries.iter().cloned());
        let access_form = Form::list(access_items, place.span);
        let store = self.fresh_setf_temporary(place.span);
        let outer_store_form = Form::list(
            vec![
                Form::atom("SETF", place.span),
                access_form.clone(),
                store.clone(),
            ],
            place.span,
        );
        let nested_store_form = Form::list(
            vec![
                Form::atom("LET", place.span),
                Form::list(
                    vec![Form::list(
                        vec![
                            nested.store.clone(),
                            outer_temporaries[container_index].clone(),
                        ],
                        place.span,
                    )],
                    place.span,
                ),
                nested.store_form.clone(),
            ],
            place.span,
        );
        let store_form = Form::list(
            vec![
                Form::atom("PROGN", place.span),
                outer_store_form,
                nested_store_form,
            ],
            place.span,
        );

        Ok(SetfExpansion {
            temporaries,
            values,
            store,
            store_form,
            access_form,
        })
    }

    fn modify_macro_container_index(operator: &str, argument_count: usize) -> Option<usize> {
        let normalized = intern_name(operator);
        let candidate = package::split_symbol(normalized.as_ref())
            .map_or_else(|| normalized.as_ref(), |(_, symbol, _)| symbol);
        let index = if [
            "CAR",
            "FIRST",
            "CDR",
            "REST",
            "GETF",
            "ELT",
            "CHAR",
            "SCHAR",
            "BIT",
            "AREF",
            "ROW-MAJOR-AREF",
            "SVREF",
            "SUBSEQ",
        ]
        .iter()
        .any(|name| names_equal(candidate, name))
        {
            0
        } else if names_equal(candidate, "NTH") {
            1
        } else {
            return None;
        };
        (index < argument_count).then_some(index)
    }
}
