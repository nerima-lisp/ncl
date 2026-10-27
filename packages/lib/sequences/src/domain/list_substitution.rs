//! Non-destructive Common Lisp `SUBST` tree substitution.

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

fn matches<'scope, C: FunctionCaller>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    caller: &mut C,
    options: RootedOptions<'scope>,
    old: Handle<'scope>,
    candidate: Handle<'scope>,
) -> Result<bool, ObjectError> {
    let keyed = if let Some(key) = options.key {
        let argument = scope.get(candidate).as_word();
        let arguments = scope.root_many(&[Local::from_word(argument)]);
        let mut values = MultipleValues::new();
        scope.call_function(runtime, key, &arguments, caller, &mut values)?
    } else {
        candidate
    };

    if let Some(test) = options.test.or(options.test_not) {
        let old_word = scope.get(old).as_word();
        let keyed_word = scope.get(keyed).as_word();
        let arguments =
            scope.root_many(&[Local::from_word(old_word), Local::from_word(keyed_word)]);
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
        scope.get(old).as_word(),
        scope.get(keyed).as_word(),
    ))
}

fn rebuild<'scope>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    mut tail: Handle<'scope>,
    mut frames: Vec<ConsFrame<'scope>>,
) -> Result<Handle<'scope>, ObjectError> {
    while let Some(frame) = frames.pop() {
        let head_unchanged =
            scope.get(frame.substituted_head).as_word() == scope.get(frame.original_head).as_word();
        let tail_unchanged = scope.get(tail).as_word() == scope.get(frame.original_tail).as_word();
        tail = if head_unchanged && tail_unchanged {
            frame.original
        } else {
            scope.make_cons(runtime, frame.substituted_head, tail)?
        };
    }
    Ok(tail)
}

fn subst_tree<'scope, C: FunctionCaller>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    caller: &mut C,
    replacement: Handle<'scope>,
    old: Handle<'scope>,
    tree: Handle<'scope>,
    options: RootedOptions<'scope>,
) -> Result<Handle<'scope>, ObjectError> {
    let mut cursor = tree;
    let mut frames = Vec::new();
    loop {
        if matches(scope, runtime, caller, options, old, cursor)? {
            return rebuild(scope, runtime, replacement, frames);
        }

        let current = scope.get(cursor).as_word();
        if !current.is_cons() {
            return rebuild(scope, runtime, cursor, frames);
        }

        let head_word = car(scope.context(), current)?;
        let tail_word = cdr(scope.context(), current)?;
        let original_head = scope.root(Local::from_word(head_word));
        let original_tail = scope.root(Local::from_word(tail_word));
        let substituted_head = subst_tree(
            scope,
            runtime,
            caller,
            replacement,
            old,
            original_head,
            options,
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
    let mut scope = Scope::new(ctx);
    let replacement = scope.root(Local::from_word(replacement));
    let old = scope.root(Local::from_word(old));
    let tree = scope.root(Local::from_word(tree));
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
    let result = subst_tree(&mut scope, runtime, caller, replacement, old, tree, options)?;
    Ok(scope.get(result).as_word())
}
