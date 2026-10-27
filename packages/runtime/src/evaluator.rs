use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

use ncl_object::{CodeObject, HeapRootedWord, Runtime as ObjectRuntime, ThreadContext, Word};
use ncl_sys::{CodePtr, RootToken};

use crate::PublishedFunction;

pub struct EvaluatorState {
    pub(crate) code: RefCell<Vec<CodePtr>>,
    pub(crate) undefined_function_stub: usize,
    pub(crate) functions: RefCell<BTreeMap<u32, PublishedFunction>>,
    pub(crate) entry_codes: RefCell<BTreeMap<usize, (Box<Word>, RootToken)>>, // check-added-lines: allow(word-table) each code object remains rooted until Runtime::drop
    rooted_functions: RefCell<Vec<(Box<Word>, RootToken)>>,
    pub(crate) pending_undefined_function_condition: RefCell<Option<HeapRootedWord>>,
}

#[derive(Clone)]
pub struct EvaluatorService {
    pub(crate) state: Rc<EvaluatorState>,
}

impl fmt::Debug for EvaluatorService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EvaluatorService")
            .finish_non_exhaustive()
    }
}

impl EvaluatorService {
    pub(crate) fn new(code: Vec<CodePtr>, undefined_function_stub: usize) -> Self {
        Self {
            state: Rc::new(EvaluatorState {
                code: RefCell::new(code),
                undefined_function_stub,
                functions: RefCell::new(BTreeMap::new()),
                entry_codes: RefCell::new(BTreeMap::new()),
                rooted_functions: RefCell::new(Vec::new()),
                pending_undefined_function_condition: RefCell::new(None),
            }),
        }
    }

    pub(crate) fn pending_undefined_function_condition(&self) -> Option<Word> {
        self.state
            .pending_undefined_function_condition
            .borrow()
            .as_ref()
            .map(HeapRootedWord::get)
    }

    pub(crate) fn clear_pending_undefined_function_condition(&self) {
        *self.state.pending_undefined_function_condition.borrow_mut() = None;
    }

    pub(crate) fn root_entry_code(&self, context: &mut ThreadContext, entry: usize, code: Word) {
        let mut rooted = Box::new(code);
        let token = ncl_object::push_root(context, &mut rooted);
        self.state
            .entry_codes
            .borrow_mut()
            .insert(entry, (rooted, token));
    }

    pub(crate) fn root_function(&self, context: &mut ThreadContext, function: Word) {
        let mut rooted = Box::new(function);
        let token = ncl_object::push_root(context, &mut rooted);
        self.state
            .rooted_functions
            .borrow_mut()
            .push((rooted, token));
    }

    pub(crate) fn entry_code(&self, entry: usize) -> Option<Word> {
        self.state
            .entry_codes
            .borrow()
            .get(&entry)
            .map(|(word, _)| **word)
    }

    pub(crate) fn push_code(&self, code: CodePtr) {
        self.state.code.borrow_mut().push(code);
    }

    pub(crate) fn clear_roots(&self, context: &mut ThreadContext) {
        let entry_codes = std::mem::take(&mut *self.state.entry_codes.borrow_mut());
        for (_, (_, token)) in entry_codes {
            let _ = ncl_object::pop_root(context, token);
        }
        let rooted_functions = std::mem::take(&mut *self.state.rooted_functions.borrow_mut());
        for (_, token) in rooted_functions.into_iter().rev() {
            let _ = ncl_object::pop_root(context, token);
        }
    }
}

impl EntryCodeLookup for EvaluatorService {
    fn code_object_word(&self, entry: usize) -> Option<Word> {
        self.entry_code(entry)
    }
}

/// Returns a copied rooted code-object word so lookup borrows end before callbacks allocate.
pub trait EntryCodeLookup {
    /// Resolve an entry to a copied code-object word before allocation can run.
    fn code_object_word(&self, entry: usize) -> Option<Word>;
}

// check-added-lines: allow(word-table) legacy callers retain each code object's RootToken
impl EntryCodeLookup for BTreeMap<usize, (Box<Word>, RootToken)> {
    fn code_object_word(&self, entry: usize) -> Option<Word> {
        self.get(&entry).map(|(word, _)| **word)
    }
}

pub struct NativeInvocation<'a> {
    pub(crate) object: &'a ObjectRuntime,
    pub(crate) context: &'a mut ThreadContext,
    pub(crate) code: CodeObject,
    pub(crate) entry_codes: &'a dyn EntryCodeLookup,
}

impl NativeInvocation<'_> {
    pub(crate) fn evaluator_access(&self) -> Option<Rc<EvaluatorAccess>> {
        self.context.evaluator_access::<EvaluatorAccess>()
    }
}

/// Typed services available to native callbacks during an evaluator invocation.
pub struct EvaluatorAccess {
    pub(crate) evaluator: EvaluatorService,
    pub(crate) object: Rc<ObjectRuntime>,
}

impl EvaluatorAccess {
    pub(crate) const fn new(evaluator: EvaluatorService, object: Rc<ObjectRuntime>) -> Self {
        Self { evaluator, object }
    }
}

impl crate::Runtime {
    pub(crate) fn with_evaluator_access<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, crate::RuntimeError>,
    ) -> Result<T, crate::RuntimeError> {
        self.context
            .set_condition_handler_invoker(crate::function_call::invoke_condition_handler);
        let evaluator = std::ptr::from_mut(self).cast();
        let previous_evaluator = self.context.replace_evaluator_runtime(evaluator);
        let previous_access =
            self.context
                .replace_evaluator_access(Some(Rc::new(EvaluatorAccess::new(
                    self.evaluator.clone(),
                    Rc::clone(&self.object),
                ))));
        let result = operation(self);
        self.clear_pending_undefined_function_condition();
        self.context.restore_evaluator_runtime(previous_evaluator);
        self.context.restore_evaluator_access(previous_access);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::{CodeObject, EvaluatorService, NativeInvocation, Word};
    use crate::evaluator::EntryCodeLookup;
    use ncl_object::Function;
    use std::collections::BTreeMap;
    use std::ptr::NonNull;

    #[test]
    fn evaluator_service_debug_reports_its_type() {
        let evaluator = EvaluatorService::new(Vec::new(), 0);

        assert_eq!(format!("{evaluator:?}"), "EvaluatorService { .. }");
    }

    #[test]
    fn evaluator_service_looks_up_rooted_entry_codes() {
        let mut context = ncl_object::ThreadContext::new();
        let evaluator = EvaluatorService::new(Vec::new(), 0);
        evaluator.root_entry_code(&mut context, 17, Word::TRUE);
        evaluator.root_function(&mut context, Word::NIL);

        assert_eq!(evaluator.code_object_word(17), Some(Word::TRUE));
        assert_eq!(evaluator.code_object_word(18), None);

        evaluator.clear_roots(&mut context);
    }

    #[test]
    fn legacy_entry_code_lookup_reads_rooted_entries() {
        let mut context = ncl_object::ThreadContext::new();
        let mut word = Box::new(Word::TRUE);
        let token = ncl_object::push_root(&mut context, &mut word);
        let entries = BTreeMap::from([(17, (word, token))]);

        assert_eq!(entries.code_object_word(17), Some(Word::TRUE));
        assert_eq!(entries.code_object_word(18), None);
        assert!(ncl_object::pop_root(&mut context, token));
    }

    #[test]
    fn native_make_closure_uses_legacy_entry_code_after_service_miss() {
        let mut runtime = crate::Runtime::new().expect("runtime");
        let entry = 0x1000_usize;
        let fallback_code = ncl_object::make_code_object(
            &mut runtime.context,
            &runtime.object,
            entry,
            0,
            Word::NIL,
            Word::NIL,
            Word::NIL,
        )
        .expect("fallback code object");
        let mut fallback_word = Box::new(fallback_code.as_word());
        let fallback_token = ncl_object::push_root(&mut runtime.context, &mut fallback_word);
        let caller_code = ncl_object::make_code_object(
            &mut runtime.context,
            &runtime.object,
            entry + 1,
            0,
            Word::NIL,
            Word::NIL,
            Word::NIL,
        )
        .expect("caller code object");
        let mut caller_word = Box::new(caller_code.as_word());
        let caller_token = ncl_object::push_root(&mut runtime.context, &mut caller_word);
        let entries = BTreeMap::from([(entry, (fallback_word, fallback_token))]);

        runtime
            .with_evaluator_access(|runtime| {
                let closure = {
                    let mut invocation = NativeInvocation {
                        object: &runtime.object,
                        context: &mut runtime.context,
                        code: CodeObject::from_word(*caller_word),
                        entry_codes: &entries,
                    };
                    let thread = invocation.context.thread_access().as_non_null();
                    let invocation_pointer = NonNull::from(&mut invocation).cast();
                    let previous =
                        ncl_sys::replace_native_context(thread, Some(invocation_pointer));
                    let closure = crate::support::native_make_closure(
                        thread,
                        Word::fixnum(0),
                        Word::from_bits(u64::try_from(entry).expect("entry fits in a word")),
                    );
                    assert_eq!(
                        ncl_sys::replace_native_context(thread, previous),
                        Some(invocation_pointer)
                    );
                    closure
                };
                let function = Function::from_word(closure);
                let closure_code = ncl_object::function_code(&runtime.context, function)
                    .expect("closure code object");
                assert_eq!(closure_code.as_word(), *entries[&entry].0);
                Ok(())
            })
            .expect("evaluator access");

        assert!(ncl_object::pop_root(&mut runtime.context, caller_token));
        assert!(ncl_object::pop_root(&mut runtime.context, fallback_token));
    }
}
