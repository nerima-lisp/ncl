#![expect(clippy::unwrap_used, reason = "Tests assert on reader behavior.")]

//! Reader acceptance tests: macro characters, dispatch characters, numbers,
//! readtable case, package prefixes, errors, and feature evaluation.

use ncl_object::{
    Bignum, ObjectRef, Package, Ratio, Runtime, ThreadContext, Word, bignum_limbs, car, cdr,
    classify, classify_object, make_cons, make_string, ratio_denominator, string_length,
    string_ref, symbol_name, symbol_package,
};
use ncl_packages::PackageListOperations;
use ncl_reader::{ReadOptions, ReadtableCase, read, read_from_string};

const EVAL_PARAMETERS: &[ncl_object::Parameter] = &[ncl_object::Parameter {
    name: ncl_object::BuiltinName::new("FORM"),
    ty: ncl_object::ParameterType::Any,
}];

fn standard(runtime: &Runtime, ctx: &mut ThreadContext) -> ReadOptions {
    ReadOptions::standard(ctx, runtime).unwrap()
}

fn read_one(runtime: &Runtime, ctx: &mut ThreadContext, text: &str) -> Word {
    let opts = standard(runtime, ctx);
    read_from_string(ctx, runtime, text, &opts)
        .unwrap()
        .unwrap()
}

fn bounded_runtime() -> Runtime {
    let runtime = Runtime::new().unwrap();
    let mut config = runtime.gc_config();
    config.dynamic_space_size = 16 * 1024 * 1024;
    config.bytes_considered_between_gcs = 64 * 1024 * 1024;
    drop(runtime);
    Runtime::with_config(config).unwrap()
}

fn name_of(ctx: &ThreadContext, word: Word) -> String {
    let name = symbol_name(ctx, word).unwrap();
    let length = string_length(ctx, name).unwrap();
    (0..length)
        .map(|i| string_ref(ctx, name, i).unwrap())
        .collect()
}

fn read_name(runtime: &Runtime, ctx: &mut ThreadContext, text: &str) -> String {
    let word = read_one(runtime, ctx, text);
    name_of(ctx, word)
}

fn register_eval_stub(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    evaluator_sentinel: &mut (),
    function: ncl_object::RustBuiltin,
) {
    let evaluator = ncl_object::Builtin {
        lambda_list: ncl_object::LambdaList::new(EVAL_PARAMETERS, &[], None, &[], false),
        convention: ncl_object::BuiltinConvention::Direct(ncl_object::Arity::exact(1)),
    };
    runtime
        .register_builtin(
            ctx,
            ncl_object::BuiltinIdentifier::new(
                ncl_object::BuiltinPackage::CommonLisp,
                ncl_object::BuiltinName::new("EVAL"),
            ),
            ncl_object::BuiltinImplementation::direct(evaluator, function),
        )
        .unwrap();
    ctx.set_evaluator_runtime(std::ptr::from_mut(evaluator_sentinel));
}

fn set_common_lisp_value(ctx: &mut ThreadContext, runtime: &Runtime, name: &str, value: Word) {
    let common_lisp = runtime.find_package(ctx, "COMMON-LISP").unwrap();
    let (symbol, _) = Package::from_word(common_lisp)
        .intern(ctx, runtime, name)
        .unwrap();
    let symbol = ncl_object::HeapRootedWord::new(ctx, symbol).unwrap();
    ncl_object::set_symbol_value(ctx, symbol.get(), value).unwrap();
}

fn set_read_base_to_form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &ncl_object::BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ncl_object::ObjectError> {
    let form = args.required(0)?;
    let common_lisp = runtime
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ncl_object::ObjectError::Layout)?;
    let (symbol, _) = Package::from_word(common_lisp).intern(ctx, runtime, "*READ-BASE*")?;
    let symbol = ncl_object::HeapRootedWord::new(ctx, symbol)?;
    ncl_object::set_symbol_value(ctx, symbol.get(), form)?;
    Ok(form)
}

fn fill_heap_with_rooted_objects(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    roots: &mut [Word],
) -> Result<usize, ncl_object::ObjectError> {
    let mut next_root = 1;
    for words in [65_536, 4_096, 256, 16, 1] {
        loop {
            if next_root == roots.len() {
                return Err(ncl_object::ObjectError::Layout);
            }
            match ncl_object::allocate(ctx, runtime, 0x7f, words) {
                Ok(value) => {
                    let root = roots
                        .get_mut(next_root)
                        .ok_or(ncl_object::ObjectError::Layout)?;
                    *root = value;
                    next_root += 1;
                }
                Err(error) if format!("{error:?}") == "Storage(CapacityExceeded)" => break,
                Err(error) => return Err(error),
            }
        }
    }
    Ok(next_root)
}

fn fill_heap_into_root_vector(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    root_vector: Word,
) -> Result<usize, ncl_object::ObjectError> {
    let mut next_root = 0;
    for words in [65_536, 4_096, 256, 16, 1] {
        loop {
            if next_root == 1_024 {
                return Err(ncl_object::ObjectError::Layout);
            }
            match ncl_object::allocate(ctx, runtime, 0x7f, words) {
                Ok(value) => {
                    ncl_object::simple_vector_set(ctx, root_vector, next_root, value)?;
                    next_root += 1;
                }
                Err(error) if format!("{error:?}") == "Storage(CapacityExceeded)" => break,
                Err(error) => return Err(error),
            }
        }
    }
    Ok(next_root)
}

fn exhaust_heap_and_return_form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &ncl_object::BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ncl_object::ObjectError> {
    let form = args.required(0)?;
    let common_lisp = runtime
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ncl_object::ObjectError::Layout)?;
    let (suppression, _) =
        Package::from_word(common_lisp).intern(ctx, runtime, "*READ-SUPPRESS*")?;
    let suppression = ncl_object::HeapRootedWord::new(ctx, suppression)?;
    let values = vec![Word::NIL; 1_024];
    let root_vector = ncl_object::make_simple_vector(ctx, runtime, &values)?;
    let root_vector = ncl_object::HeapRootedWord::new(ctx, root_vector)?;
    let root_count = fill_heap_into_root_vector(ctx, runtime, root_vector.get())?;
    assert!(root_count > 1);
    ncl_object::set_symbol_value(ctx, suppression.get(), root_vector.get())?;
    Ok(form)
}

fn list_names(ctx: &ThreadContext, list: Word) -> Vec<String> {
    let mut names = Vec::new();
    let mut cursor = list;
    while cursor != Word::NIL {
        let item = car(ctx, cursor).unwrap();
        names.push(name_of(ctx, item));
        cursor = cdr(ctx, cursor).unwrap();
    }
    names
}

fn read_names(runtime: &Runtime, ctx: &mut ThreadContext, text: &str) -> Vec<String> {
    let list = read_one(runtime, ctx, text);
    list_names(ctx, list)
}

#[path = "reader/bounded_heap.rs"]
mod bounded_heap;
#[path = "reader/labels.rs"]
mod labels;
#[path = "reader/numbers.rs"]
mod numbers;
#[path = "reader/read_eval.rs"]
mod read_eval;
#[path = "reader/syntax.rs"]
mod syntax;
