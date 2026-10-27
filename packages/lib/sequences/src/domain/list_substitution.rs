//! Common Lisp tree substitution functions.

use ncl_object::{
    FunctionCaller, Handle, Local, MultipleValues, ObjectError, Runtime, Scope, ThreadContext,
    Word, car, cdr,
};

use super::selection::SelectionOptions;

#[derive(Clone, Copy)]
struct RootedOptions<'scope> {
    key: Option<Handle<'scope>>,
    test: Option<Handle<'scope>>,
    test_not: Option<Handle<'scope>>,
}

struct ConsFrame<'scope> {
    original: Handle<'scope>,
    original_head: Handle<'scope>,
    original_tail: Handle<'scope>,
    substituted_head: Handle<'scope>,
}

#[derive(Clone, Copy)]
enum RootedMatcher<'scope> {
    Equal {
        replacement: Handle<'scope>,
        old: Handle<'scope>,
    },
    Predicate {
        replacement: Handle<'scope>,
        predicate: Handle<'scope>,
        negate: bool,
    },
    Alist {
        alist: Handle<'scope>,
    },
}

#[derive(Clone, Copy)]
enum Matcher {
    Equal {
        replacement: Word,
        old: Word,
    },
    Predicate {
        replacement: Word,
        predicate: Word,
        negate: bool,
    },
    Alist(Word),
}

fn matches<'scope, C: FunctionCaller>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    caller: &mut C,
    matcher: RootedMatcher<'scope>,
    options: RootedOptions<'scope>,
    candidate: Handle<'scope>,
) -> Result<Option<Handle<'scope>>, ObjectError> {
    let keyed = if let Some(key) = options.key {
        let argument = scope.get(candidate).as_word();
        let arguments = scope.root_many(&[Local::from_word(argument)]);
        let mut values = MultipleValues::new();
        scope.call_function(runtime, key, &arguments, caller, &mut values)?
    } else {
        candidate
    };

    match matcher {
        RootedMatcher::Equal { replacement, old } => {
            let test_passed = test_pair(scope, runtime, caller, options, old, keyed)?;
            Ok(test_passed.then_some(replacement))
        }
        RootedMatcher::Predicate {
            replacement,
            predicate,
            negate,
        } => {
            let argument = scope.get(keyed).as_word();
            let arguments = scope.root_many(&[Local::from_word(argument)]);
            let mut values = MultipleValues::new();
            let result =
                scope.call_function(runtime, predicate, &arguments, caller, &mut values)?;
            let predicate_passed = (scope.get(result).as_word() != Word::NIL) != negate;
            Ok(predicate_passed.then_some(replacement))
        }
        RootedMatcher::Alist { alist } => {
            let mut cursor = alist;
            loop {
                let cursor_word = scope.get(cursor).as_word();
                if cursor_word == Word::NIL {
                    return Ok(None);
                }

                let entry_word = car(scope.context(), cursor_word)?;
                if entry_word != Word::NIL {
                    let key_word = car(scope.context(), entry_word)?;
                    let key = scope.root(Local::from_word(key_word));
                    let replacement_word = cdr(scope.context(), entry_word)?;
                    let replacement = scope.root(Local::from_word(replacement_word));
                    if test_pair(scope, runtime, caller, options, key, keyed)? {
                        return Ok(Some(replacement));
                    }
                }

                let tail_word = cdr(scope.context(), cursor_word)?;
                cursor = scope.root(Local::from_word(tail_word));
            }
        }
    }
}

fn test_pair<'scope, C: FunctionCaller>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    caller: &mut C,
    options: RootedOptions<'scope>,
    left: Handle<'scope>,
    right: Handle<'scope>,
) -> Result<bool, ObjectError> {
    if let Some(test) = options.test.or(options.test_not) {
        let left_word = scope.get(left).as_word();
        let right_word = scope.get(right).as_word();
        let arguments =
            scope.root_many(&[Local::from_word(left_word), Local::from_word(right_word)]);
        let mut values = MultipleValues::new();
        let result = scope.call_function(runtime, test, &arguments, caller, &mut values)?;
        let truth = scope.get(result).as_word() != Word::NIL;
        return Ok(if options.test_not.is_some() {
            !truth
        } else {
            truth
        });
    }

    Ok(ncl_lib_numbers::eql_words(
        scope.context(),
        scope.get(left).as_word(),
        scope.get(right).as_word(),
    ))
}

fn rebuild<'scope>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    mut tail: Handle<'scope>,
    mut frames: Vec<ConsFrame<'scope>>,
    destructive: bool,
) -> Result<Handle<'scope>, ObjectError> {
    while let Some(frame) = frames.pop() {
        let head_unchanged =
            scope.get(frame.substituted_head).as_word() == scope.get(frame.original_head).as_word();
        let tail_unchanged = scope.get(tail).as_word() == scope.get(frame.original_tail).as_word();
        if destructive {
            let cons = scope.get(frame.original).as_word();
            if !head_unchanged {
                let head = scope.get(frame.substituted_head).as_word();
                ncl_object::rplaca(scope.context_mut(), cons, head)?;
            }
            if !tail_unchanged {
                let tail_word = scope.get(tail).as_word();
                ncl_object::rplacd(scope.context_mut(), cons, tail_word)?;
            }
            tail = frame.original;
        } else {
            tail = if head_unchanged && tail_unchanged {
                frame.original
            } else {
                scope.make_cons(runtime, frame.substituted_head, tail)?
            };
        }
    }
    Ok(tail)
}

fn subst_tree<'scope, C: FunctionCaller>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    caller: &mut C,
    matcher: RootedMatcher<'scope>,
    tree: Handle<'scope>,
    options: RootedOptions<'scope>,
    destructive: bool,
) -> Result<Handle<'scope>, ObjectError> {
    let mut cursor = tree;
    let mut frames = Vec::new();
    loop {
        if let Some(replacement) = matches(scope, runtime, caller, matcher, options, cursor)? {
            return rebuild(scope, runtime, replacement, frames, destructive);
        }

        let current = scope.get(cursor).as_word();
        if !current.is_cons() {
            return rebuild(scope, runtime, cursor, frames, destructive);
        }

        let head_word = car(scope.context(), current)?;
        let tail_word = cdr(scope.context(), current)?;
        let original_head = scope.root(Local::from_word(head_word));
        let original_tail = scope.root(Local::from_word(tail_word));
        let substituted_head = subst_tree(
            scope,
            runtime,
            caller,
            matcher,
            original_head,
            options,
            destructive,
        )?;
        frames.push(ConsFrame {
            original: cursor,
            original_head,
            original_tail,
            substituted_head,
        });
        cursor = original_tail;
    }
}

fn substitute_tree<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    matcher: Matcher,
    tree: Word,
    options: SelectionOptions,
    destructive: bool,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let tree = scope.root(Local::from_word(tree));
    let matcher = match matcher {
        Matcher::Equal { replacement, old } => RootedMatcher::Equal {
            replacement: scope.root(Local::from_word(replacement)),
            old: scope.root(Local::from_word(old)),
        },
        Matcher::Predicate {
            replacement,
            predicate,
            negate,
        } => RootedMatcher::Predicate {
            replacement: scope.root(Local::from_word(replacement)),
            predicate: scope.root(Local::from_word(predicate)),
            negate,
        },
        Matcher::Alist(alist) => RootedMatcher::Alist {
            alist: scope.root(Local::from_word(alist)),
        },
    };
    let key = options
        .key
        .filter(|key| *key != Word::NIL)
        .map(|key| scope.root(Local::from_word(key)));
    let test = options.test.map(|test| scope.root(Local::from_word(test)));
    let test_not = options
        .test_not
        .map(|test_not| scope.root(Local::from_word(test_not)));
    let options = RootedOptions {
        key,
        test,
        test_not,
    };
    let result = subst_tree(
        &mut scope,
        runtime,
        caller,
        matcher,
        tree,
        options,
        destructive,
    )?;
    Ok(scope.get(result).as_word())
}

/// Substitute `replacement` for every matching subtree or leaf in `tree`.
///
/// # Errors
/// Returns an object error when a test, key, or allocation fails.
pub fn subst<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    replacement: Word,
    old: Word,
    tree: Word,
    options: SelectionOptions,
) -> Result<Word, ObjectError> {
    substitute_tree(
        ctx,
        runtime,
        caller,
        Matcher::Equal { replacement, old },
        tree,
        options,
        false,
    )
}

pub fn nsubst<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    replacement: Word,
    old: Word,
    tree: Word,
    options: SelectionOptions,
) -> Result<Word, ObjectError> {
    substitute_tree(
        ctx,
        runtime,
        caller,
        Matcher::Equal { replacement, old },
        tree,
        options,
        true,
    )
}

pub fn subst_if<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    replacement: Word,
    predicate: Word,
    tree: Word,
    options: SelectionOptions,
) -> Result<Word, ObjectError> {
    substitute_tree(
        ctx,
        runtime,
        caller,
        Matcher::Predicate {
            replacement,
            predicate,
            negate: false,
        },
        tree,
        options,
        false,
    )
}

pub fn subst_if_not<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    replacement: Word,
    predicate: Word,
    tree: Word,
    options: SelectionOptions,
) -> Result<Word, ObjectError> {
    substitute_tree(
        ctx,
        runtime,
        caller,
        Matcher::Predicate {
            replacement,
            predicate,
            negate: true,
        },
        tree,
        options,
        false,
    )
}

pub fn nsubst_if<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    replacement: Word,
    predicate: Word,
    tree: Word,
    options: SelectionOptions,
) -> Result<Word, ObjectError> {
    substitute_tree(
        ctx,
        runtime,
        caller,
        Matcher::Predicate {
            replacement,
            predicate,
            negate: false,
        },
        tree,
        options,
        true,
    )
}

pub fn nsubst_if_not<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    replacement: Word,
    predicate: Word,
    tree: Word,
    options: SelectionOptions,
) -> Result<Word, ObjectError> {
    substitute_tree(
        ctx,
        runtime,
        caller,
        Matcher::Predicate {
            replacement,
            predicate,
            negate: true,
        },
        tree,
        options,
        true,
    )
}

pub fn sublis<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    alist: Word,
    tree: Word,
    options: SelectionOptions,
) -> Result<Word, ObjectError> {
    substitute_tree(
        ctx,
        runtime,
        caller,
        Matcher::Alist(alist),
        tree,
        options,
        false,
    )
}

pub fn nsublis<C: FunctionCaller>(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    caller: &mut C,
    alist: Word,
    tree: Word,
    options: SelectionOptions,
) -> Result<Word, ObjectError> {
    substitute_tree(
        ctx,
        runtime,
        caller,
        Matcher::Alist(alist),
        tree,
        options,
        true,
    )
}
