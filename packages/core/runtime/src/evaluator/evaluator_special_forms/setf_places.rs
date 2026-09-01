#![allow(clippy::wildcard_imports)]
use super::*;

mod aref_place;
mod bit_place;
mod element_places;
mod list_places;
mod property_places;
#[cfg(test)]
mod property_places_tests;
mod slot_places;
mod subseq_places;
mod symbol_cell_places;
mod vector_places;

impl Runtime {
    pub(crate) fn set_place(
        &self,
        place: &Form,
        value: Value,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if let Some(expanded) = Self::expand_symbol_macro_form(place, environment)? {
            return self.set_place(&expanded, value, environment);
        }
        if atom_name(place).is_some() {
            let (resolved_name, escaped) =
                Self::variable_name_info(place, "SETF target must be a symbol")?;
            self.set_or_define_variable_in(
                &resolved_name,
                escaped,
                value,
                environment,
                place.span,
            )?;
            return Ok(());
        }

        let FormKind::List(items) = &place.kind else {
            return Err(Self::invalid("unsupported SETF place", place.span));
        };
        let Some(operator) = items.first().and_then(atom_name) else {
            return Err(Self::invalid("unsupported SETF place", place.span));
        };
        let args = &items[1..];

        let lookup_name = unqualified_name(operator);
        if environment.lookup_setf_expander(&lookup_name).is_some() {
            let expansion = self.get_setf_expansion(place, environment)?;
            return self.apply_setf_expansion(&expansion, value, environment, place.span);
        }
        if let Some(Value::Function(function)) = self.lookup_function_in(&lookup_name, environment)
            && self
                .set_function_place(
                    function.as_ref(),
                    args,
                    value.clone(),
                    environment,
                    place.span,
                )?
                .is_some()
        {
            return Ok(());
        }

        if let Some(updater) = environment.lookup_setf_function(&lookup_name) {
            let mut arguments = args
                .iter()
                .map(|argument| self.eval_in(argument, environment))
                .collect::<Result<Vec<_>, _>>()?;
            arguments.push(value);
            self.apply_in(&updater, &arguments, place.span, environment)?;
            return Ok(());
        }

        match lookup_name.as_str() {
            "LDB" => {
                if args.len() != 2 {
                    return Err(Self::invalid("LDB SETF place needs a byte specifier and place", place.span));
                }
                let byte_spec = self.eval_in(&args[0], environment)?;
                let old_value = self.eval_in(&args[1], environment)?;
                let updated = crate::builtins::dpb(
                    &[value, byte_spec, old_value],
                )?;
                self.set_place(&args[1], updated, environment)
            }
            "MASK-FIELD" => {
                if args.len() != 2 {
                    return Err(Self::invalid(
                        "MASK-FIELD SETF place needs a byte specifier and place",
                        place.span,
                    ));
                }
                let byte_spec = self.eval_in(&args[0], environment)?;
                let old_value = self.eval_in(&args[1], environment)?;
                let updated = crate::builtins::deposit_field(&[value, byte_spec, old_value])?;
                self.set_place(&args[1], updated, environment)
            }
            "SLOT-VALUE" => self.set_slot_value_place(args, value, environment, place.span),
            "CAR" | "FIRST" | "CDR" | "REST" | "NTH" | "SECOND" | "THIRD" | "FOURTH"
            | "FIFTH" | "SIXTH" | "SEVENTH" | "EIGHTH" | "NINTH" | "TENTH" => self
                .set_list_place(lookup_name.as_str(), args, value, environment, place.span)
                .map(|_| ()),
            "ELT" | "CHAR" | "SCHAR" => {
                self.set_element_place(lookup_name.as_str(), args, value, environment, place.span)
            }
            "SUBSEQ" => self.set_subseq_place(args, &value, environment, place.span),
            "SVREF" | "ROW-MAJOR-AREF" => self.set_vector_index_place(
                lookup_name.as_str(),
                args,
                value,
                environment,
                place.span,
            ),
            "AREF" => self.set_aref_place(args, value, environment, place.span),
            "BIT" => self.set_bit_place(args, value, environment, place.span),
            "SYMBOL-VALUE" | "SYMBOL-FUNCTION" => self.set_symbol_cell_place(
                lookup_name.as_str(),
                args,
                value,
                environment,
                place.span,
            ),
            "GET" | "GETHASH" | "GETF" => {
                self.set_property_place(lookup_name.as_str(), args, value, environment)
            }
            _ => Err(Self::invalid("unsupported SETF place", place.span)),
        }
    }
}
