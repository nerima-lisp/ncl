use ncl_object::{
    BuiltinFunctionCaller, FunctionArguments, FunctionCaller, FunctionDesignator, MultipleValues,
    ObjectError, Package, Runtime, ThreadContext, Word, car, cdr, simple_vector_ref,
    structure_layout, symbol_plist, with_root, with_roots,
};

pub(super) fn invoke_structure_print_function(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
    stream: Word,
) -> Result<bool, ObjectError> {
    with_roots(ctx, &[object, stream], |ctx, roots| {
        let object = roots.first().ok_or(ObjectError::Layout)?.get();
        let layout = structure_layout(ctx, object)?;
        let Some(class) = runtime.structure_class(ctx, layout) else {
            return Ok(false);
        };
        let mut class = class;
        with_root(ctx, &mut class, |ctx, class| {
            let mut name = simple_vector_ref(ctx, class.get(), 0)?;
            with_root(ctx, &mut name, |ctx, name| {
                let mut key = Package::from_word(runtime.ensure_package(ctx, "NCL")?)
                    .intern(ctx, runtime, "%STRUCTURE-PRINT-FUNCTION")?
                    .0;
                with_root(ctx, &mut key, |ctx, key| {
                    let mut plist = symbol_plist(ctx, name.get())?;
                    with_root(ctx, &mut plist, |ctx, plist| {
                        let mut plist_word = plist.get();
                        while plist_word != Word::NIL {
                            let (found, next) = with_root(ctx, &mut plist_word, |ctx, plist| {
                                let mut value_cell = cdr(ctx, plist.get())?;
                                with_root(ctx, &mut value_cell, |ctx, value_cell| {
                                    let found = if car(ctx, plist.get())? == key.get() {
                                        let mut function = car(ctx, value_cell.get())?;
                                        with_root(ctx, &mut function, |ctx, function| {
                                            let mut function_word = function.get();
                                            if function.get().is_cons() {
                                                let mut operator = car(ctx, function.get())?;
                                                with_root(ctx, &mut operator, |ctx, operator| {
                                                    let common_lisp = runtime
                                                        .ensure_package(ctx, "COMMON-LISP")?;
                                                    let function_operator =
                                                        Package::from_word(common_lisp)
                                                            .intern(ctx, runtime, "FUNCTION")?
                                                            .0;
                                                    if operator.get() == function_operator {
                                                        function_word =
                                                            car(ctx, cdr(ctx, function.get())?)?;
                                                    } else {
                                                        function_word = function.get();
                                                    }
                                                    Ok(())
                                                })?;
                                            }
                                            let designator = FunctionDesignator::try_from_word(
                                                ctx,
                                                function_word,
                                            )?;
                                            // This hook runs before write_to_string traverses the object, so it receives depth 0.
                                            let words = [
                                                roots.first().ok_or(ObjectError::Layout)?.get(),
                                                roots.get(1).ok_or(ObjectError::Layout)?.get(),
                                                Word::fixnum(0),
                                            ];
                                            let mut caller = BuiltinFunctionCaller;
                                            let mut values = MultipleValues::new();
                                            caller.call_function(
                                                ctx,
                                                runtime,
                                                designator,
                                                FunctionArguments::new(&words),
                                                &mut values,
                                            )?;
                                            Ok(true)
                                        })
                                    } else {
                                        Ok(false)
                                    }?;
                                    Ok((found, cdr(ctx, value_cell.get())?))
                                })
                            })?;
                            if found {
                                return Ok(true);
                            }
                            plist_word = next;
                        }
                        Ok(false)
                    })
                })
            })
        })
    })
}
