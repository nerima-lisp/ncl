use super::reservations::CompilationReservations;
use super::{
    Environment, Form, FormKind, Runtime, RuntimeError, SetfExpansion, Value, atom_name,
    unqualified_name,
};

impl Runtime {
    pub(super) fn prepare_compiled_place_mutation(
        &self,
        items: &mut [Form],
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        let operator = items.first().and_then(atom_name).map(unqualified_name);
        if matches!(
            operator.as_deref(),
            Some("SETF" | "PSETF" | "%SETF-INTRINSIC-STORE")
        ) {
            return self.prepare_compiled_setf(items, environment);
        }
        let end = items.len().saturating_sub(1);
        for (index, operand) in items.iter_mut().enumerate().skip(1) {
            let place = match operator.as_deref() {
                Some("ROTATEF") => true,
                Some("SHIFTF") => index < end,
                Some("POP") => index == 1,
                _ => index == 2,
            };
            *operand = if place {
                self.prepare_compiled_place(operand, environment, true)?
            } else {
                self.prepare_compiled_form(operand, environment)?
            };
        }
        Ok(())
    }

    pub(super) fn prepare_compiled_setf(
        &self,
        items: &mut [Form],
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        let intrinsic = items.first().and_then(atom_name) == Some("%SETF-INTRINSIC-STORE");
        for (index, operand) in items.iter_mut().enumerate().skip(1) {
            *operand = if index % 2 == 1 {
                if intrinsic {
                    // Terminal stores already have captured operands; expanding them
                    // as places again would recursively generate another store.
                    self.prepare_compiled_place_arguments(operand, environment)?
                } else {
                    self.prepare_compiled_place(operand, environment, false)?
                }
            } else {
                self.prepare_compiled_form(operand, environment)?
            };
        }
        Ok(())
    }

    fn prepare_compiled_place(
        &self,
        place: &Form,
        environment: &Environment,
        read_access: bool,
    ) -> Result<Form, RuntimeError> {
        if let Some(expanded) = Self::expand_symbol_macro_form(place, environment)? {
            return self.prepare_compiled_place(&expanded, environment, read_access);
        }
        let FormKind::List(items) = &place.kind else {
            return Ok(place.clone());
        };
        if items.first().and_then(atom_name) == Some("%SETF-PREPARED-EXPANSION") {
            return Ok(place.clone());
        }
        if let Some(expansion) = self.custom_setf_expansion(place, items, environment)? {
            return self.prepare_compiled_place_expansion(
                place,
                expansion,
                environment,
                read_access,
            );
        }
        let expanded = self.expand_macros(place.clone(), environment)?;
        if expanded != *place {
            return self.prepare_compiled_place(&expanded, environment, read_access);
        }
        if matches!(
            items
                .first()
                .and_then(atom_name)
                .map(unqualified_name)
                .as_deref(),
            Some("GETF" | "VALUES")
        ) {
            let expansion = self.parallel_setf_expansion_for_compilation(place, environment)?;
            return self.prepare_compiled_place_expansion(
                place,
                expansion,
                environment,
                read_access,
            );
        }
        self.prepare_compiled_place_arguments(place, environment)
    }

    fn prepare_compiled_place_arguments(
        &self,
        place: &Form,
        environment: &Environment,
    ) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &place.kind else {
            return Ok(place.clone());
        };
        let mut prepared = items.clone();
        self.prepare_tail(&mut prepared, 1, environment)?;
        Ok(Form::list(prepared, place.span))
    }

    fn prepare_compiled_place_expansion(
        &self,
        place: &Form,
        mut expansion: SetfExpansion,
        environment: &Environment,
        read_access: bool,
    ) -> Result<Form, RuntimeError> {
        let mut reservations = CompilationReservations::new(self);
        for form in expansion
            .temporaries
            .iter()
            .chain(&expansion.values)
            .chain(&expansion.stores)
            .chain([&expansion.store_form, &expansion.access_form])
        {
            reservations.reserve(form);
        }
        let local = environment.child();
        for (temporary, value) in expansion.temporaries.iter().zip(&mut expansion.values) {
            *value = self.prepare_compiled_form(value, &local)?;
            Self::bind_compiled_setf_variable(temporary, &local)?;
        }
        if read_access {
            expansion.access_form = self.prepare_compiled_form(&expansion.access_form, &local)?;
        }
        for store in &expansion.stores {
            Self::bind_compiled_setf_variable(store, &local)?;
        }
        expansion.store_form = self.prepare_compiled_form(&expansion.store_form, &local)?;
        // Preserve syntax until the expander selects the evaluated forms, while
        // lexical macros still exist. Execution must not invoke the expander again.
        Ok(Form::list(
            vec![
                Form::atom("%SETF-PREPARED-EXPANSION", place.span),
                Form::list(expansion.temporaries, place.span),
                Form::list(expansion.values, place.span),
                Form::list(expansion.stores, place.span),
                expansion.store_form,
                expansion.access_form,
            ],
            place.span,
        ))
    }

    fn bind_compiled_setf_variable(
        variable: &Form,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        let (name, escaped) =
            Self::variable_name_info(variable, "SETF expansion binding must be a symbol")?;
        if escaped {
            environment.define_exact(name, Value::Nil);
        } else {
            environment.define(name, Value::Nil);
        }
        Ok(())
    }
}
