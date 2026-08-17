impl Runtime {
    pub(crate) fn set_place(
        &self,
        place: &Form,
        value: Value,
        environment: &Environment,
    ) -> Result<(), RuntimeError> {
        if let Some(expanded) = self.expand_symbol_macro_form(place, environment)? {
            return self.set_place(&expanded, value, environment);
        }
        if atom_name(place).is_some() {
            let (resolved_name, escaped) =
                self.variable_name_info(place, "SETF target must be a symbol")?;
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
            return Err(self.invalid("unsupported SETF place", place.span));
        };
        let Some(operator) = items.first().and_then(atom_name) else {
            return Err(self.invalid("unsupported SETF place", place.span));
        };
        let args = &items[1..];
        let lookup_name = unqualified_name(operator);

        if environment.lookup_setf_expander(&lookup_name).is_some() {
            let expansion = self.get_setf_expansion(place, environment)?;
            return self.apply_setf_expansion(&expansion, value, environment, place.span);
        }
        if let Some(Value::Function(function)) = self.lookup_function_in(&lookup_name, environment)
            && let Some(result) =
                self.set_accessor_place(function.as_ref(), args, &value, place, environment)
        {
            return result;
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

        if let Some(result) =
            self.set_sequence_place(&lookup_name, args, &value, place, environment)
        {
            return result;
        }
        if let Some(result) = self.set_array_place(&lookup_name, args, &value, place, environment) {
            return result;
        }
        if let Some(result) = self.set_object_place(&lookup_name, args, &value, place, environment)
        {
            return result;
        }
        if let Some(result) = self.set_symbol_place(&lookup_name, args, &value, place, environment)
        {
            return result;
        }

        Err(self.invalid("unsupported SETF place", place.span))
    }
}

include!("places/accessors.rs");
include!("places/sequences.rs");
include!("places/arrays.rs");
include!("places/objects.rs");
include!("places/symbols.rs");
