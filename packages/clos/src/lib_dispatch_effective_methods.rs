use super::{
    COMMON_LISP, Local, MethodCombination, MultipleValues, ObjectError, Runtime, Scope, Word,
    call_method, dispatch_list_to_handles, dispatch_push_handle, symbol_function,
    symbol_name_string, tail_handles,
};

#[expect(
    clippy::too_many_arguments,
    reason = "Standard method combination receives each method group and the active call state separately."
)]
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

#[expect(
    clippy::too_many_arguments,
    reason = "Short method combination receives its operator, name, primary methods, and active call state."
)]
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

pub fn invoke_custom_effective_method<'ctx, C: ncl_object::FunctionCaller>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    builder: ncl_object::Handle<'ctx>,
    groups: &ncl_object::HandleVec<'ctx>,
    argument_list: ncl_object::Handle<'ctx>,
    caller: &mut C,
    values: &mut MultipleValues,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let groups = scope.make_list(runtime, groups)?;
    let builder_arguments = scope.root_many(&[
        Local::from_word(scope.get(groups).as_word()),
        Local::from_word(Word::NIL),
    ]);
    let effective_form =
        scope.call_function(runtime, builder, &builder_arguments, caller, values)?;
    let effective_form = scope.root(Local::from_word(scope.get(effective_form).as_word()));
    invoke_randomized_effective_form(
        scope,
        runtime,
        effective_form,
        argument_list,
        caller,
        values,
    )
}

fn invoke_randomized_effective_form<'ctx, C: ncl_object::FunctionCaller>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    effective_form: ncl_object::Handle<'ctx>,
    argument_list: ncl_object::Handle<'ctx>,
    caller: &mut C,
    values: &mut MultipleValues,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let loop_form = method_combination_form(scope, effective_form, "LOOP")?;
    let catch_form = *loop_form.as_slice().get(1).ok_or(ObjectError::TypeError)?;
    let catch_form = method_combination_form(scope, catch_form, "CATCH")?;
    let return_form = *catch_form.as_slice().get(2).ok_or(ObjectError::TypeError)?;
    let return_form = method_combination_form(scope, return_form, "RETURN")?;
    let rcase_form = *return_form
        .as_slice()
        .get(1)
        .ok_or(ObjectError::TypeError)?;
    let clauses = method_combination_form(scope, rcase_form, "RCASE")?;
    let clause_handles = clauses.as_slice().get(1..).ok_or(ObjectError::TypeError)?;
    let mut weights = scope.root_many(&[]);
    for clause in clause_handles.iter().copied() {
        let fields = dispatch_list_to_handles(scope, clause)?;
        let weight = *fields.as_slice().first().ok_or(ObjectError::TypeError)?;
        dispatch_push_handle(scope, &mut weights, weight);
    }
    let plus_name = scope.intern(runtime, COMMON_LISP, "+")?;
    let plus = scope.root(Local::from_word(symbol_function(
        scope.context(),
        scope.get(plus_name).as_word(),
    )?));
    let total = scope.call_function(runtime, plus, &weights, caller, values)?;
    let random_name = scope.intern(runtime, COMMON_LISP, "RANDOM")?;
    let random = scope.root(Local::from_word(symbol_function(
        scope.context(),
        scope.get(random_name).as_word(),
    )?));
    let random_args = scope.root_many(&[Local::from_word(scope.get(total).as_word())]);
    let target = scope.call_function(runtime, random, &random_args, caller, values)?;
    let less_name = scope.intern(runtime, COMMON_LISP, "<")?;
    let less = scope.root(Local::from_word(symbol_function(
        scope.context(),
        scope.get(less_name).as_word(),
    )?));
    let mut cumulative: ncl_object::Handle<'_, Word> =
        scope.root(Local::from_word(Word::fixnum(0)));
    for clause in clause_handles
        .iter()
        .copied()
        .take(clause_handles.len().saturating_sub(1))
    {
        let fields = dispatch_list_to_handles(scope, clause)?;
        let weight = *fields.as_slice().first().ok_or(ObjectError::TypeError)?;
        let sum_args = scope.root_many(&[
            Local::from_word(scope.get(cumulative).as_word()),
            Local::from_word(scope.get(weight).as_word()),
        ]);
        let sum = scope.call_function(runtime, plus, &sum_args, caller, values)?;
        cumulative = scope.root(Local::from_word(scope.get(sum).as_word()));
        let compare_args = scope.root_many(&[
            Local::from_word(scope.get(target).as_word()),
            Local::from_word(scope.get(cumulative).as_word()),
        ]);
        let comparison = scope.call_function(runtime, less, &compare_args, caller, values)?;
        if scope.get(comparison).as_word() != Word::NIL {
            return invoke_call_method_clause(
                scope,
                runtime,
                clause,
                argument_list,
                caller,
                values,
            );
        }
    }
    let last = clause_handles
        .last()
        .copied()
        .ok_or(ObjectError::UndefinedFunction)?;
    invoke_call_method_clause(scope, runtime, last, argument_list, caller, values)
}

fn method_combination_form<'ctx>(
    scope: &mut Scope<'ctx>,
    form: ncl_object::Handle<'ctx>,
    expected: &str,
) -> Result<ncl_object::HandleVec<'ctx>, ObjectError> {
    let fields = dispatch_list_to_handles(scope, form)?;
    let operator = *fields.as_slice().first().ok_or(ObjectError::TypeError)?;
    if symbol_name_string(scope.context(), scope.get(operator).as_word())? != expected {
        return Err(ObjectError::TypeError);
    }
    Ok(fields)
}

fn invoke_call_method_clause<'ctx, C: ncl_object::FunctionCaller>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    clause: ncl_object::Handle<'ctx>,
    argument_list: ncl_object::Handle<'ctx>,
    caller: &mut C,
    values: &mut MultipleValues,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let clause = dispatch_list_to_handles(scope, clause)?;
    let call_form = *clause.as_slice().get(1).ok_or(ObjectError::TypeError)?;
    let call = method_combination_form(scope, call_form, "CALL-METHOD")?;
    let method_entry = *call.as_slice().get(1).ok_or(ObjectError::TypeError)?;
    let method_fields = dispatch_list_to_handles(scope, method_entry)?;
    let function = *method_fields
        .as_slice()
        .get(2)
        .ok_or(ObjectError::TypeError)?;
    let next_entries = call.as_slice().get(2).copied();
    let mut next_functions = scope.root_many(&[]);
    if let Some(next_entries) = next_entries {
        let next_entries = dispatch_list_to_handles(scope, next_entries)?;
        for entry in next_entries.iter().copied() {
            let fields = dispatch_list_to_handles(scope, entry)?;
            let function = *fields.as_slice().get(2).ok_or(ObjectError::TypeError)?;
            dispatch_push_handle(scope, &mut next_functions, function);
        }
    }
    let next = scope.make_list(runtime, &next_functions)?;
    let arguments = dispatch_list_to_handles(scope, argument_list)?;
    call_method(
        scope,
        runtime,
        function,
        &arguments,
        argument_list,
        next,
        caller,
        values,
    )
}
