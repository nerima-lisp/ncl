use super::{
    Local, MethodCombination, MultipleValues, ObjectError, Runtime, Scope, Word, call_method,
    dispatch_push_handle, symbol_function, tail_handles,
};

#[allow(clippy::too_many_arguments)]
pub fn invoke_standard_effective_method<'ctx, C: ncl_object::FunctionCaller>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    before: &ncl_object::HandleVec<'ctx>,
    primary: &ncl_object::HandleVec<'ctx>,
    after: &ncl_object::HandleVec<'ctx>,
    arguments: &ncl_object::HandleVec<'ctx>,
    argument_list: ncl_object::Handle<'ctx>,
    caller: &mut C,
    values: &mut MultipleValues,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    for function in before.iter().copied() {
        let nil = scope.root(Local::from_word(Word::NIL));
        call_method(
            scope,
            runtime,
            function,
            arguments,
            argument_list,
            nil,
            caller,
            values,
        )?;
    }
    let first = *primary
        .as_slice()
        .first()
        .ok_or(ObjectError::UndefinedFunction)?;
    let rest = tail_handles(scope, primary);
    let next = scope.make_list(runtime, &rest)?;
    let result = call_method(
        scope,
        runtime,
        first,
        arguments,
        argument_list,
        next,
        caller,
        values,
    )?;
    let returned_values = scope.root_many(
        &values
            .as_slice()
            .iter()
            .copied()
            .map(Local::<Word>::from_word)
            .collect::<Vec<_>>(),
    );
    for function in after.iter().copied() {
        let nil = scope.root(Local::from_word(Word::NIL));
        call_method(
            scope,
            runtime,
            function,
            arguments,
            argument_list,
            nil,
            caller,
            values,
        )?;
    }
    let returned = returned_values
        .iter()
        .map(|value| scope.get(*value).as_word())
        .collect::<Vec<_>>();
    values.set(&returned);
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub fn invoke_short_effective_method<'ctx, C: ncl_object::FunctionCaller>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    combination: MethodCombination,
    combination_name: ncl_object::Handle<'ctx>,
    primary: &ncl_object::HandleVec<'ctx>,
    arguments: &ncl_object::HandleVec<'ctx>,
    argument_list: ncl_object::Handle<'ctx>,
    caller: &mut C,
    values: &mut MultipleValues,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let nil = scope.root(Local::from_word(Word::NIL));
    match combination {
        MethodCombination::And => {
            let mut last = None;
            for function in primary.iter().copied() {
                let result = call_method(
                    scope,
                    runtime,
                    function,
                    arguments,
                    argument_list,
                    nil,
                    caller,
                    values,
                )?;
                if scope.get(result).as_word() == Word::NIL {
                    return Ok(result);
                }
                last = Some(result);
            }
            last.ok_or(ObjectError::UndefinedFunction)
        }
        MethodCombination::Or => {
            let mut last = None;
            for function in primary.iter().copied() {
                let result = call_method(
                    scope,
                    runtime,
                    function,
                    arguments,
                    argument_list,
                    nil,
                    caller,
                    values,
                )?;
                if scope.get(result).as_word() != Word::NIL {
                    return Ok(result);
                }
                last = Some(result);
            }
            last.ok_or(ObjectError::UndefinedFunction)
        }
        MethodCombination::Progn => {
            let mut last = None;
            for function in primary.iter().copied() {
                last = Some(call_method(
                    scope,
                    runtime,
                    function,
                    arguments,
                    argument_list,
                    nil,
                    caller,
                    values,
                )?);
            }
            last.ok_or(ObjectError::UndefinedFunction)
        }
        MethodCombination::Plus
        | MethodCombination::Append
        | MethodCombination::List
        | MethodCombination::Max
        | MethodCombination::Min
        | MethodCombination::Nconc => {
            let mut results = scope.root_many(&[]);
            for function in primary.iter().copied() {
                let result = call_method(
                    scope,
                    runtime,
                    function,
                    arguments,
                    argument_list,
                    nil,
                    caller,
                    values,
                )?;
                let result = scope.root(Local::from_word(scope.get(result).as_word()));
                dispatch_push_handle(scope, &mut results, result);
            }
            let operator_word =
                symbol_function(scope.context(), scope.get(combination_name).as_word())?;
            let operator = scope.root(Local::from_word(operator_word));
            scope.call_function(runtime, operator, &results, caller, values)
        }
        MethodCombination::Standard => Err(ObjectError::TypeError),
    }
}
