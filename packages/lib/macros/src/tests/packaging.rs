use super::*;

fn expand(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    adapter: ncl_object::RustBuiltin,
    input: Word,
) -> Result<Word> {
    let input_args = [input];
    let args = ncl_object::BuiltinArgs::new(&input_args);
    adapter(ctx, runtime, &args, &mut MultipleValues::new())
}

fn head(ctx: &mut ThreadContext, form: Word) -> Result<Word> {
    crate::form::elements(ctx, form)?
        .first()
        .copied()
        .ok_or(ObjectError::TypeError)
}

#[test]
fn defpackage_expands_to_progn_of_creation_use_shadow_and_export() -> Result<()> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let defpackage = symbol(&mut ctx, &runtime, "DEFPACKAGE")?;
    let name = symbol(&mut ctx, &runtime, "CL-BENCH")?;
    let use_clause = {
        let use_head = symbol(&mut ctx, &runtime, "USE")?;
        let cl = symbol(&mut ctx, &runtime, "CL")?;
        list(&mut ctx, &runtime, &[use_head, cl])?
    };
    let export_clause = {
        let export_head = symbol(&mut ctx, &runtime, "EXPORT")?;
        let bench_run = symbol(&mut ctx, &runtime, "BENCH-RUN")?;
        list(&mut ctx, &runtime, &[export_head, bench_run])?
    };
    let form = list(
        &mut ctx,
        &runtime,
        &[defpackage, name, use_clause, export_clause],
    )?;

    let expansion = expand(&mut ctx, &runtime, defpackage_adapter, form)?;
    assert_eq!(
        head(&mut ctx, expansion)?,
        symbol(&mut ctx, &runtime, "PROGN")?
    );
    let elements = crate::form::elements(&mut ctx, expansion)?;
    // PROGN, OR-creation, USE-PACKAGE, EXPORT, and the trailing FIND-PACKAGE.
    assert_eq!(elements.len(), 5);
    let statements = &elements[1..];
    assert_eq!(
        head(&mut ctx, statements[0])?,
        symbol(&mut ctx, &runtime, "OR")?
    );
    assert_eq!(
        head(&mut ctx, statements[1])?,
        symbol(&mut ctx, &runtime, "USE-PACKAGE")?
    );
    assert_eq!(
        head(&mut ctx, statements[2])?,
        symbol(&mut ctx, &runtime, "EXPORT")?
    );
    assert_eq!(
        head(&mut ctx, statements[3])?,
        symbol(&mut ctx, &runtime, "FIND-PACKAGE")?
    );
    Ok(())
}

#[test]
fn defpackage_rejects_unknown_options() -> Result<()> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let defpackage = symbol(&mut ctx, &runtime, "DEFPACKAGE")?;
    let name = symbol(&mut ctx, &runtime, "CL-TEST")?;
    let bad_clause = {
        let head = symbol(&mut ctx, &runtime, "BOGUS")?;
        let text = symbol(&mut ctx, &runtime, "IGNORED")?;
        list(&mut ctx, &runtime, &[head, text])?
    };
    let form = list(&mut ctx, &runtime, &[defpackage, name, bad_clause])?;
    assert_eq!(
        expand(&mut ctx, &runtime, defpackage_adapter, form),
        Err(ObjectError::TypeError)
    );
    Ok(())
}

#[test]
fn defpackage_expands_clhs_package_clauses() -> Result<()> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let defpackage = symbol(&mut ctx, &runtime, "DEFPACKAGE")?;
    let name = symbol(&mut ctx, &runtime, "CLHS-CLAUSES")?;
    let clause = |ctx: &mut ThreadContext, head: &str, args: &[Word]| -> Result<Word> {
        let mut values = vec![symbol(ctx, &runtime, head)?];
        values.extend_from_slice(args);
        list(ctx, &runtime, &values)
    };
    let source = symbol(&mut ctx, &runtime, "COMMON-LISP")?;
    let imported = symbol(&mut ctx, &runtime, "CAR")?;
    let shadowed = symbol(&mut ctx, &runtime, "CDR")?;
    let interned = symbol(&mut ctx, &runtime, "LOCAL")?;
    let documentation =
        ncl_object::make_string(&mut ctx, &runtime, &"docs".chars().collect::<Vec<_>>())?;
    let size = Word::fixnum(32);
    let import_from = clause(&mut ctx, "IMPORT-FROM", &[source, imported])?;
    let shadowing_import_from = clause(&mut ctx, "SHADOWING-IMPORT-FROM", &[source, shadowed])?;
    let intern = clause(&mut ctx, "INTERN", &[interned])?;
    let documentation_clause = clause(&mut ctx, "DOCUMENTATION", &[documentation])?;
    let size_clause = clause(&mut ctx, "SIZE", &[size])?;
    let form = list(
        &mut ctx,
        &runtime,
        &[
            defpackage,
            name,
            import_from,
            shadowing_import_from,
            intern,
            documentation_clause,
            size_clause,
        ],
    )?;

    let expansion = expand(&mut ctx, &runtime, defpackage_adapter, form)?;
    let elements = crate::form::elements(&mut ctx, expansion)?;
    let statements = &elements[1..];
    assert_eq!(
        head(&mut ctx, statements[0])?,
        symbol(&mut ctx, &runtime, "OR")?
    );

    let make_package = crate::form::elements(&mut ctx, statements[0])?[2];
    let make_package = crate::form::elements(&mut ctx, make_package)?;
    assert_eq!(make_package.len(), 6);
    assert_eq!(designator_text(&ctx, make_package[2])?, "DOCUMENTATION");
    assert_eq!(designator_text(&ctx, make_package[4])?, "SIZE");

    let statement_heads = statements
        .iter()
        .map(|statement| head(&mut ctx, *statement))
        .collect::<Result<Vec<_>>>()?;
    let expected = ["OR", "INTERN", "IMPORT", "SHADOWING-IMPORT", "FIND-PACKAGE"];
    assert_eq!(statement_heads.len(), expected.len());
    for (actual, expected) in statement_heads.iter().zip(expected) {
        assert_eq!(*actual, symbol(&mut ctx, &runtime, expected)?);
    }
    Ok(())
}

#[test]
fn in_package_expands_to_setq_of_star_package_star() -> Result<()> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let in_package = symbol(&mut ctx, &runtime, "IN-PACKAGE")?;
    let name = symbol(&mut ctx, &runtime, "CL-BENCH")?;
    let form = list(&mut ctx, &runtime, &[in_package, name])?;
    let expansion = expand(&mut ctx, &runtime, in_package_adapter, form)?;
    assert_eq!(
        head(&mut ctx, expansion)?,
        symbol(&mut ctx, &runtime, "SETQ")?
    );
    let parts = crate::form::elements(&mut ctx, expansion)?;
    assert_eq!(
        parts.get(1).copied(),
        Some(symbol(&mut ctx, &runtime, "*PACKAGE*")?)
    );
    Ok(())
}

/// Every `held_*` helper allocates while other, already-built sub-forms are
/// still needed. Under `gc_stress` (a collection before every allocation)
/// and `strict_forwarding` (panic on a stale, unforwarded `Word`), a form
/// this size exercises many held-vector growth/refresh cycles.
///
/// The input form itself is built with `gc_stress` off (building it is not
/// under test, and building composite forms one un-rooted local at a time,
/// as this test does, is itself an example of the anti-pattern the rooting
/// rules ban); the stress flags are only turned on around the call that
/// exercises `packaging.rs`'s `held_*` helpers.
#[test]
fn defpackage_survives_gc_stress_and_strict_forwarding() -> Result<()> {
    let runtime = Runtime::new()?;
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)?;

    let defpackage = symbol(&mut ctx, &runtime, "DEFPACKAGE")?;
    let name = symbol(&mut ctx, &runtime, "STRESS-PACKAGE")?;
    let use_clause = {
        let mut items = vec![symbol(&mut ctx, &runtime, "USE")?];
        items.push(symbol(&mut ctx, &runtime, "CL")?);
        list(&mut ctx, &runtime, &items)?
    };
    let nicknames_clause = {
        let mut items = vec![symbol(&mut ctx, &runtime, "NICKNAMES")?];
        for index in 0..4 {
            items.push(symbol(&mut ctx, &runtime, &format!("STRESS-NICK-{index}"))?);
        }
        list(&mut ctx, &runtime, &items)?
    };
    let shadow_clause = {
        let mut items = vec![symbol(&mut ctx, &runtime, "SHADOW")?];
        for index in 0..4 {
            items.push(symbol(
                &mut ctx,
                &runtime,
                &format!("STRESS-SHADOW-{index}"),
            )?);
        }
        list(&mut ctx, &runtime, &items)?
    };
    let export_clause = {
        let mut items = vec![symbol(&mut ctx, &runtime, "EXPORT")?];
        for index in 0..8 {
            items.push(symbol(
                &mut ctx,
                &runtime,
                &format!("STRESS-EXPORT-{index}"),
            )?);
        }
        list(&mut ctx, &runtime, &items)?
    };
    let mut form = list(
        &mut ctx,
        &runtime,
        &[
            defpackage,
            name,
            use_clause,
            nicknames_clause,
            shadow_clause,
            export_clause,
        ],
    )?;

    let form_token = ncl_object::push_root(&mut ctx, &mut form);
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);
    let expansion = expand(&mut ctx, &runtime, defpackage_adapter, form);
    ctx.set_gc_stress(false);
    assert!(ncl_object::pop_root(&mut ctx, form_token));
    let expansion = expansion?;
    assert_eq!(
        head(&mut ctx, expansion)?,
        symbol(&mut ctx, &runtime, "PROGN")?
    );
    Ok(())
}
