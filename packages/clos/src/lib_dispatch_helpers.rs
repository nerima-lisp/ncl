type EffectiveMethod<'ctx> = ncl_object::Handle<'ctx>;

fn dispatch_list_to_handles<'ctx>(
    scope: &mut Scope<'ctx>,
    handle: ncl_object::Handle<'ctx>,
) -> Result<ncl_object::HandleVec<'ctx>, ObjectError> {
    let word = scope.get(handle).as_word();
    scope.list_to_handle_vec(Local::from_word(word))
}

fn dispatch_push_handle<'ctx>(
    scope: &mut Scope<'ctx>,
    values: &mut ncl_object::HandleVec<'ctx>,
    handle: ncl_object::Handle<'ctx>,
) {
    let word = scope.get(handle).as_word();
    values.push(scope, Local::from_word(word));
}

fn tail_handles<'ctx>(
    scope: &mut Scope<'ctx>,
    values: &ncl_object::HandleVec<'ctx>,
) -> ncl_object::HandleVec<'ctx> {
    let words = values
        .as_slice()
        .get(1..)
        .unwrap_or(&[])
        .iter()
        .map(|handle| scope.get(*handle).as_word())
        .collect::<Vec<_>>();
    scope.root_many(
        &words
            .iter()
            .copied()
            .map(Local::from_word)
            .collect::<Vec<_>>(),
    )
}

#[cfg(test)]
#[path = "../tests/support/lib_dispatch_helpers_tests.rs"]
mod dispatch_helper_tests;
