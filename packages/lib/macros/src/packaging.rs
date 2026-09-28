//! Expanders for `DEFPACKAGE` and `IN-PACKAGE`.
//!
//! Both macros rewrite their form into ordinary calls on the package
//! builtins registered by `ncl-lib-packages` (`MAKE-PACKAGE`,
//! `FIND-PACKAGE`, `USE-PACKAGE`, `SHADOW`, `EXPORT`, `IMPORT`,
//! `SHADOWING-IMPORT`, `INTERN`, and `FIND-SYMBOL`). Neither
//! macro performs a heap mutation itself; like the other expanders in this
//! crate, they only build a new form for the evaluator to run.
//!
//! Every intermediate value that survives an allocation (building one
//! sub-form can allocate cons cells, a symbol, or a string while an earlier
//! sub-form's `Word` is still needed) is kept inside a single `held: Vec<Word>`
//! accumulator that is re-rooted through [`ncl_object::with_roots`] on every
//! allocating step, mirroring `iteration.rs`'s `held_form` idiom.

#![allow(clippy::redundant_pub_crate)]

use ncl_object::{MultipleValues, ObjectError, ObjectRef, Runtime, ThreadContext, Word};

use crate::defining::{form_elements, required};
use crate::form::{list, symbol};

type Result<T = Word> = std::result::Result<T, ObjectError>;

fn held_value(held: &[Word], index: usize) -> Result<Word> {
    held.get(index).copied().ok_or(ObjectError::TypeError)
}

#[derive(Debug)]
struct PackageClauses {
    use_indexes: Vec<usize>,
    export_indexes: Vec<usize>,
    shadow_indexes: Vec<usize>,
    nickname_indexes: Vec<usize>,
    intern_indexes: Vec<usize>,
    import_from: Vec<(usize, Vec<usize>)>,
    shadowing_import_from: Vec<(usize, Vec<usize>)>,
    documentation: Option<usize>,
    size: Option<usize>,
}

fn collect_package_clauses(
    ctx: &mut ThreadContext,
    held: &mut Vec<Word>,
) -> Result<PackageClauses> {
    let mut clauses = PackageClauses {
        use_indexes: Vec::new(),
        export_indexes: Vec::new(),
        shadow_indexes: Vec::new(),
        nickname_indexes: Vec::new(),
        intern_indexes: Vec::new(),
        import_from: Vec::new(),
        shadowing_import_from: Vec::new(),
        documentation: None,
        size: None,
    };
    for clause_index in 2..held.len() {
        let clause = held_value(held, clause_index)?;
        let clause_parts = form_elements(ctx, clause)?;
        let clause_head = clause_parts
            .first()
            .copied()
            .ok_or(ObjectError::TypeError)?;
        let head_text = designator_text(ctx, clause_head)?;
        let rest = clause_parts.get(1..).unwrap_or(&[]);
        let base = held.len();
        held.extend_from_slice(rest);
        let indexes: Vec<usize> = (base..held.len()).collect();
        match head_text.as_str() {
            "USE" => clauses.use_indexes.extend(indexes),
            "EXPORT" => clauses.export_indexes.extend(indexes),
            "SHADOW" => clauses.shadow_indexes.extend(indexes),
            "NICKNAMES" => clauses.nickname_indexes.extend(indexes),
            "INTERN" => clauses.intern_indexes.extend(indexes),
            "IMPORT-FROM" => {
                let package = indexes.first().copied().ok_or(ObjectError::TypeError)?;
                clauses
                    .import_from
                    .push((package, indexes.into_iter().skip(1).collect()));
            }
            "SHADOWING-IMPORT-FROM" => {
                let package = indexes.first().copied().ok_or(ObjectError::TypeError)?;
                clauses
                    .shadowing_import_from
                    .push((package, indexes.into_iter().skip(1).collect()));
            }
            "DOCUMENTATION" => {
                let index = indexes.first().copied().ok_or(ObjectError::TypeError)?;
                if indexes.len() != 1 || clauses.documentation.replace(index).is_some() {
                    return Err(ObjectError::TypeError);
                }
            }
            "SIZE" => {
                let index = indexes.first().copied().ok_or(ObjectError::TypeError)?;
                if indexes.len() != 1 || clauses.size.replace(index).is_some() {
                    return Err(ObjectError::TypeError);
                }
            }
            _ => return Err(ObjectError::TypeError),
        }
    }
    Ok(clauses)
}

fn append_import_statements(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    statements: &mut Vec<Word>,
    quoted_name: Word,
    clauses: &PackageClauses,
) -> Result<()> {
    for (source_package, names) in clauses.import_from.iter().cloned() {
        let source_package = held_quote(ctx, runtime, held, source_package)?;
        let mut imported = Vec::with_capacity(names.len());
        for name in names {
            let quoted_name = held_quote(ctx, runtime, held, name)?;
            imported.push(held_form(
                ctx,
                runtime,
                held,
                "FIND-SYMBOL",
                &[quoted_name, source_package],
            )?);
        }
        let imported = held_form(ctx, runtime, held, "LIST", &imported)?;
        statements.push(held_form(
            ctx,
            runtime,
            held,
            "IMPORT",
            &[imported, quoted_name],
        )?);
    }
    for (source_package, names) in clauses.shadowing_import_from.iter().cloned() {
        let source_package = held_quote(ctx, runtime, held, source_package)?;
        let mut imported = Vec::with_capacity(names.len());
        for name in names {
            let quoted_name = held_quote(ctx, runtime, held, name)?;
            imported.push(held_form(
                ctx,
                runtime,
                held,
                "FIND-SYMBOL",
                &[quoted_name, source_package],
            )?);
        }
        let imported = held_form(ctx, runtime, held, "LIST", &imported)?;
        statements.push(held_form(
            ctx,
            runtime,
            held,
            "SHADOWING-IMPORT",
            &[imported, quoted_name],
        )?);
    }
    Ok(())
}

/// Verify that `held[0]` is the symbol `name`, re-rooting `held` across the
/// (possibly allocating, first-use) lookup of that symbol.
///
/// `defining::ensure_form_operator` cannot be reused here: it interns its
/// own comparison symbol without taking `held`'s current backing storage as
/// a root, so under `gc_stress` an allocation inside it can leave every
/// value already copied into `held` pointing at stale, pre-collection
/// addresses.
fn held_ensure_operator(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    name: &str,
) -> Result<()> {
    let (matches, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let operator = symbol(ctx, runtime, name)?;
        let leading = roots
            .first()
            .map(|root| **root)
            .ok_or(ObjectError::TypeError)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((leading == operator, refreshed))
    })?;
    *held = refreshed;
    if matches {
        Ok(())
    } else {
        Err(ObjectError::TypeError)
    }
}

/// Build `(operator arg-at-index...)` from already-held values and append it.
fn held_form(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    operator: &str,
    indexes: &[usize],
) -> Result<usize> {
    let (value, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let mut values = Vec::with_capacity(indexes.len() + 1);
        values.push(symbol(ctx, runtime, operator)?);
        for index in indexes {
            values.push(**roots.get(*index).ok_or(ObjectError::TypeError)?);
        }
        let value = list(ctx, runtime, &values)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((value, refreshed))
    })?;
    *held = refreshed;
    held.push(value);
    Ok(held.len() - 1)
}

/// Build the plain data list `(arg-at-index...)` (no leading operator).
fn held_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    indexes: &[usize],
) -> Result<usize> {
    let (value, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let values = indexes
            .iter()
            .map(|index| {
                roots
                    .get(*index)
                    .map(|root| **root)
                    .ok_or(ObjectError::TypeError)
            })
            .collect::<Result<Vec<_>>>()?;
        let value = list(ctx, runtime, &values)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((value, refreshed))
    })?;
    *held = refreshed;
    held.push(value);
    Ok(held.len() - 1)
}

fn held_quote(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    index: usize,
) -> Result<usize> {
    held_form(ctx, runtime, held, "QUOTE", &[index])
}

fn held_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    name: &str,
) -> Result<usize> {
    let (value, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let value = symbol(ctx, runtime, name)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((value, refreshed))
    })?;
    *held = refreshed;
    held.push(value);
    Ok(held.len() - 1)
}

fn held_string(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    text: &str,
) -> Result<usize> {
    let chars: Vec<char> = text.chars().collect();
    let (value, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let value = ncl_object::make_string(ctx, runtime, &chars)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((value, refreshed))
    })?;
    *held = refreshed;
    held.push(value);
    Ok(held.len() - 1)
}

/// The uppercase text of a symbol or string designator.
fn designator_text(ctx: &ThreadContext, word: Word) -> Result<String> {
    let text_word = match ncl_object::classify_object(ctx, word) {
        ObjectRef::Symbol(_) => ncl_object::symbol_name(ctx, word)?,
        ObjectRef::String(_) => word,
        _ => return Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) explicit error, not a swallow
    };
    (0..ncl_object::string_length(ctx, text_word)?)
        .map(|index| ncl_object::string_ref(ctx, text_word, index))
        .collect()
}

/// `(DEFPACKAGE name options...)`.
///
/// Expands to a `PROGN` that creates the package (if it does not already
/// exist), applies the package options, and returns the package.
pub(crate) fn defpackage(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut MultipleValues,
) -> Result<Word> {
    let parts = form_elements(ctx, required(args, 0)?)?;
    ncl_object::with_roots(ctx, &parts, |ctx, roots| {
        let mut held = roots.iter().map(|root| **root).collect::<Vec<_>>();
        held_ensure_operator(ctx, runtime, &mut held, "DEFPACKAGE")?;
        let name_index = 1;
        held_value(&held, name_index)?;

        let clauses = collect_package_clauses(ctx, &mut held)?;

        let quoted_name = held_quote(ctx, runtime, &mut held, name_index)?;
        let find_existing = held_form(ctx, runtime, &mut held, "FIND-PACKAGE", &[quoted_name])?;
        let mut make_indexes = vec![quoted_name];
        if !clauses.nickname_indexes.is_empty() {
            let nickname_data = held_list(ctx, runtime, &mut held, &clauses.nickname_indexes)?;
            let quoted_nicknames = held_quote(ctx, runtime, &mut held, nickname_data)?;
            let nicknames_key = held_string(ctx, runtime, &mut held, "NICKNAMES")?;
            make_indexes.extend([nicknames_key, quoted_nicknames]);
        }
        if let Some(index) = clauses.documentation {
            let key = held_string(ctx, runtime, &mut held, "DOCUMENTATION")?;
            make_indexes.extend([key, index]);
        }
        if let Some(index) = clauses.size {
            let key = held_string(ctx, runtime, &mut held, "SIZE")?;
            make_indexes.extend([key, index]);
        }
        let make_call = held_form(ctx, runtime, &mut held, "MAKE-PACKAGE", &make_indexes)?;
        let mut statements = vec![held_form(
            ctx,
            runtime,
            &mut held,
            "OR",
            &[find_existing, make_call],
        )?];

        for use_index in clauses.use_indexes {
            let quoted_use = held_quote(ctx, runtime, &mut held, use_index)?;
            statements.push(held_form(
                ctx,
                runtime,
                &mut held,
                "USE-PACKAGE",
                &[quoted_use, quoted_name],
            )?);
        }

        if !clauses.shadow_indexes.is_empty() {
            let shadow_data = held_list(ctx, runtime, &mut held, &clauses.shadow_indexes)?;
            let quoted_shadow = held_quote(ctx, runtime, &mut held, shadow_data)?;
            statements.push(held_form(
                ctx,
                runtime,
                &mut held,
                "SHADOW",
                &[quoted_shadow, quoted_name],
            )?);
        }

        for intern_index in clauses.intern_indexes {
            let quoted_intern = held_quote(ctx, runtime, &mut held, intern_index)?;
            statements.push(held_form(
                ctx,
                runtime,
                &mut held,
                "INTERN",
                &[quoted_intern, quoted_name],
            )?);
        }

        append_import_statements(
            ctx,
            runtime,
            &mut held,
            &mut statements,
            quoted_name,
            &clauses,
        )?;

        if !clauses.export_indexes.is_empty() {
            let mut interned = Vec::with_capacity(clauses.export_indexes.len());
            for export_index in clauses.export_indexes {
                let quoted_export = held_quote(ctx, runtime, &mut held, export_index)?;
                interned.push(held_form(
                    ctx,
                    runtime,
                    &mut held,
                    "INTERN",
                    &[quoted_export, quoted_name],
                )?);
            }
            let export_list = held_form(ctx, runtime, &mut held, "LIST", &interned)?;
            statements.push(held_form(
                ctx,
                runtime,
                &mut held,
                "EXPORT",
                &[export_list, quoted_name],
            )?);
        }

        statements.push(find_existing);
        held_form(ctx, runtime, &mut held, "PROGN", &statements)
            .and_then(|index| held_value(&held, index))
    })
}

/// `(IN-PACKAGE name)`.
///
/// Expands to `(SETQ *PACKAGE* (OR (FIND-PACKAGE 'name) (ERROR "...")))`.
/// The reader-side package switch (so subsequent forms in the same file
/// read package-qualified symbols correctly) is handled syntactically by
/// `ncl-runtime`'s loader; this expansion covers evaluation, including
/// `IN-PACKAGE` forms reached through any other path (`funcall`, nested
/// `progn`, etc).
pub(crate) fn in_package(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    args: &[Word],
    _values: &mut MultipleValues,
) -> Result<Word> {
    let parts = form_elements(ctx, required(args, 0)?)?;
    ncl_object::with_roots(ctx, &parts, |ctx, roots| {
        let mut held = roots.iter().map(|root| **root).collect::<Vec<_>>();
        held_ensure_operator(ctx, runtime, &mut held, "IN-PACKAGE")?;
        let name_index = 1;
        held_value(&held, name_index)?;
        let quoted_name = held_quote(ctx, runtime, &mut held, name_index)?;
        let find_call = held_form(ctx, runtime, &mut held, "FIND-PACKAGE", &[quoted_name])?;
        let message = held_string(ctx, runtime, &mut held, "package not found")?;
        let error_call = held_form(ctx, runtime, &mut held, "ERROR", &[message])?;
        let selected = held_form(ctx, runtime, &mut held, "OR", &[find_call, error_call])?;
        let variable_index = held_symbol(ctx, runtime, &mut held, "*PACKAGE*")?;
        held_form(ctx, runtime, &mut held, "SETQ", &[variable_index, selected])
            .and_then(|index| held_value(&held, index))
    })
}

pub fn defpackage_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &ncl_object::BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word> {
    let words = (0..args.len())
        .filter_map(|index| args.get(index))
        .collect::<Vec<_>>();
    defpackage(runtime, ctx, &words, values)
}

pub fn in_package_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &ncl_object::BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word> {
    let words = (0..args.len())
        .filter_map(|index| args.get(index))
        .collect::<Vec<_>>();
    in_package(runtime, ctx, &words, values)
}

#[cfg(test)]
#[path = "tests/packaging.rs"]
mod tests;
