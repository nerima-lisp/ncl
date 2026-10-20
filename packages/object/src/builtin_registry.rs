use super::{Builtin, BuiltinIdentifier, BuiltinImplementation, FunctionObject};
use crate::{Function, MultipleValues, ObjectError, Runtime, ThreadContext, function_name};
use ncl_sys::Word;

impl Runtime {
    /// Call `function` when it is registered as a builtin.
    #[must_use]
    pub fn call_builtin_if_registered(
        &self,
        ctx: &mut ThreadContext,
        function: FunctionObject,
        args: &[Word],
        values: &mut MultipleValues,
    ) -> Option<Result<Word, ObjectError>> {
        let implementation = self
            .registered_builtin(ctx, function)
            .map(|(_, implementation)| implementation);
        implementation.map(|implementation| {
            self.call_registered_builtin_with_implementation(
                ctx,
                function,
                args,
                values,
                Some(implementation),
            )
        })
    }

    pub(crate) fn registered_builtin(
        &self,
        ctx: &ThreadContext,
        function: FunctionObject,
    ) -> Option<(BuiltinIdentifier, BuiltinImplementation)> {
        let function_word = self.heap.forwarded_word(function.as_word())?;
        if ncl_sys::object_widetag(&ctx.thread, function_word) != Some(crate::widetag::SIMPLE_FUN) {
            return None;
        }
        let index = crate::object_access::get_function(
            ctx,
            function_word,
            crate::function_offset::BUILTIN_INDEX,
        )
        .ok()?
        .as_fixnum()
        .and_then(|index| usize::try_from(index).ok())?;
        let entry = {
            let builtins = self
                .builtins
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let entry = builtins
                .get(index)
                .map(|entry| (*entry.function, entry.identifier, entry.implementation));
            drop(builtins);
            entry
        };
        let (registered_function, identifier, implementation) = entry?;
        let registered_word = self.heap.forwarded_word(registered_function)?;
        (registered_word == function_word).then_some((identifier, implementation))
    }

    /// Match a serialized simple function to the newest registration for its identifier.
    #[must_use]
    pub fn registered_builtin_for_image(
        &self,
        ctx: &ThreadContext,
        name: Word,
        package: &str,
        builtin_name: &str,
    ) -> Option<FunctionObject> {
        let candidate = {
            let builtins = self
                .builtins
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            builtins
                .iter()
                .enumerate()
                .rev()
                .find(|(_, entry)| {
                    entry.identifier.package.as_str() == package
                        && entry.identifier.name.as_str() == builtin_name
                })
                .map(|(index, entry)| (index, *entry.function))
        };
        let (index, registered_function) = candidate?;
        let function_word = self.heap.forwarded_word(registered_function)?;
        if ncl_sys::object_widetag(&ctx.thread, function_word) != Some(crate::widetag::SIMPLE_FUN) {
            return None;
        }
        let index_word = crate::object_access::get_function(
            ctx,
            function_word,
            crate::function_offset::BUILTIN_INDEX,
        )
        .ok()?;
        if index_word
            .as_fixnum()
            .and_then(|value| usize::try_from(value).ok())
            != Some(index)
            || function_name(ctx, Function::from_word(function_word)).ok() != Some(name)
        {
            return None;
        }
        FunctionObject::try_from(function_word).ok()
    }

    /// Return the descriptor installed for a function object.
    #[must_use]
    pub fn builtin_descriptor(
        &self,
        ctx: &ThreadContext,
        function: FunctionObject,
    ) -> Option<Builtin> {
        self.registered_builtin(ctx, function)
            .map(|(_, implementation)| implementation.descriptor)
    }

    /// Return the stable identifier for a registered builtin function.
    #[must_use]
    pub fn builtin_identifier(
        &self,
        ctx: &ThreadContext,
        function: FunctionObject,
    ) -> Option<BuiltinIdentifier> {
        self.registered_builtin(ctx, function)
            .map(|(identifier, _)| identifier)
    }
}
