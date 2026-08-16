
fn constant_value(constant: &Constant) -> Value {
    match constant {
        Constant::Nil => Value::Nil,
        Constant::Boolean(value) => Value::boolean(*value),
        Constant::Integer(value) => Value::Integer(*value),
        Constant::Rational {
            numerator,
            denominator,
        } => Value::rational(i128::from(*numerator), i128::from(*denominator))
            .expect("compiler emitted an invalid rational constant"),
        Constant::Float(value) => Value::Float(*value),
        Constant::String(value) => Value::string(value.clone()),
        Constant::Character(value) => Value::Character(*value),
        Constant::Symbol(value) => Value::symbol(value),
        Constant::SymbolExact(value) => Value::symbol_exact(value),
        Constant::Keyword(value) => Value::keyword(value),
        Constant::KeywordExact(value) => Value::keyword_exact(value),
    }
}

fn pop_value(stack: &mut Vec<Value>, span: Span, operation: &str) -> Result<Value, RuntimeError> {
    stack
        .pop()
        .ok_or_else(|| invalid(&format!("{operation} has no value on the stack"), span))
}


fn setf_place_uses_multiple_values(place: &Form) -> bool {
    let FormKind::List(items) = &place.kind else {
        return false;
    };
    matches!(
        items.first(),
        Some(Form {
            kind: FormKind::Atom(operator),
            ..
        }) if operator == "VALUES"
    )
}
