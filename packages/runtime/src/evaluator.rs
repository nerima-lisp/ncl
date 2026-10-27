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
