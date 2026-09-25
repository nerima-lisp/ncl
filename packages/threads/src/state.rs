//! Shared helpers for the heap objects owned by the thread layer.

use std::cell::Cell;
use std::sync::{Mutex, MutexGuard};

use ncl_object::{
    Instance, Package, Runtime, ThreadContext, Word, make_instance, make_string, pop_root,
    push_root, slot_ref, slot_set,
};
use ncl_sys::RootSlot;

use crate::ThreadError;

/// Lock a mutex, recovering the value if a previous holder panicked.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Push a precise root and hand the closure an interior-mutable slot.
///
/// `ncl-object` keeps its own root helper crate-private, so this layer
/// re-establishes the same contract over the public root API.
///
pub fn with_root<T>(
    ctx: &mut ThreadContext,
    value: &mut Word,
    f: impl FnOnce(&mut ThreadContext, &Word) -> Result<T, ThreadError>,
) -> Result<T, ThreadError> {
    let mut slot = Cell::new(*value);
    let token = push_root(ctx, slot.get_mut());
    let result = {
        let root = RootSlot::new(&slot);
        f(ctx, &root)
    };
    *value = slot.get();
    if !pop_root(ctx, token) {
        return Err(ThreadError::RootStackCorrupted);
    }
    result
}

/// Intern `name` in `package`, creating the package when needed.
///
/// # Errors
/// Returns an object-layer error when the package or symbol cannot be created.
pub fn intern_internal(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    package: &str,
    name: &str,
) -> Result<Word, ThreadError> {
    let package = runtime.ensure_package(ctx, package)?;
    let (symbol, _status) = Package::from(package).intern(ctx, runtime, name)?;
    Ok(symbol)
}

/// Read an instance slot.
///
/// # Errors
/// Returns a type error when the object is not an instance, and a storage error
/// when the slot cannot be read.
pub fn read_slot(ctx: &ThreadContext, object: Word, slot: usize) -> Result<Word, ThreadError> {
    Ok(slot_ref(ctx, Instance::from(object), slot)?)
}

/// Write an instance slot.
///
/// # Errors
/// Returns a type error when the object is not an instance, and a storage error
/// when the slot cannot be written.
pub fn write_slot(
    ctx: &mut ThreadContext,
    object: Word,
    slot: usize,
    value: Word,
) -> Result<(), ThreadError> {
    slot_set(ctx, Instance::from(object), slot, value)?;
    Ok(())
}

/// Read a fixnum instance slot.
///
/// # Errors
/// Returns a type error when the object is not an instance.
pub fn read_fixnum(
    ctx: &ThreadContext,
    object: Word,
    slot: usize,
) -> Result<Option<i64>, ThreadError> {
    Ok(read_slot(ctx, object, slot)?.as_fixnum())
}

/// Read a scalar object identifier from a slot.
///
/// # Errors
/// Returns [`ThreadError::NotAThread`] when the slot does not hold a
/// non-negative fixnum.
pub fn read_handle(ctx: &ThreadContext, object: Word, slot: usize) -> Result<u64, ThreadError> {
    read_fixnum(ctx, object, slot)?
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(ThreadError::NotAThread)
}

/// Allocate an instance of a registered class with the supplied slots.
///
/// # Errors
/// Returns an object-layer error when the string or instance cannot be built.
///
pub fn make_object(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    class: &str,
    slots: &[Word],
) -> Result<Word, ThreadError> {
    let class_word = runtime.class(ctx, class).ok_or(ThreadError::MissingClass)?;
    Ok(make_instance(ctx, runtime, class_word, slots)?.as_word())
}

/// Allocate an instance whose first two slots are a name string and a handle.
///
/// # Errors
/// Returns an object-layer error when the string or instance cannot be built.
///
pub fn make_named_object(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    class: &str,
    name: &str,
    handle: u64,
    extra: &[Word],
) -> Result<Word, ThreadError> {
    let mut name_word = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
    with_root(ctx, &mut name_word, |ctx, name_word| {
        let handle = i64::try_from(handle).map_err(|_| ThreadError::NotAThread)?;
        let mut slots = Vec::with_capacity(2 + extra.len());
        slots.push(*name_word);
        slots.push(Word::fixnum(handle));
        slots.extend_from_slice(extra);
        make_object(ctx, runtime, class, &slots)
    })
}
