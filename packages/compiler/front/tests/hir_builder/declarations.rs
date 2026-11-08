use ncl_hir::{ClTypeKind, HirDeclaration, HirQuality, NodeKind};
use ncl_ir::{OpKind, Prim, ScalarTypeHint, Terminator};
use ncl_object::Word;

use super::common::{
    TestResult, context, expand_build, fixture_text, has_node, lower_both_and_verify, verify_ir,
};

#[test]
fn declarations_and_type_specifiers_retain_meaning_and_source_data() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(34);
    let integer = fixture.cl("INTEGER");
    let integer_range = fixture.list(&[integer, Word::fixnum(0), Word::fixnum(10)]);
    let x = fixture.user("X");
    let type_keyword = fixture.cl("TYPE");
    let type_declaration = fixture.list(&[type_keyword, integer_range, x]);
    let speed_name = fixture.cl("SPEED");
    let speed = fixture.list(&[speed_name, Word::fixnum(2)]);
    let safety_name = fixture.cl("SAFETY");
    let safety = fixture.list(&[safety_name, Word::fixnum(0)]);
    let optimize_name = fixture.cl("OPTIMIZE");
    let optimize = fixture.list(&[optimize_name, speed, safety]);
    let inline_name = fixture.cl("INLINE");
    let helper = fixture.user("HELPER");
    let inline = fixture.list(&[inline_name, helper]);
    let declaration = fixture.form("DECLARE", &[type_declaration, optimize, inline]);
    let assertion = fixture.form("THE", &[integer_range, x]);
    let input = fixture.form("LOCALLY", &[declaration, assertion]);
    let source = fixture_text(
        include_str!("../fixtures/hir-bridge/declarations.lisp"),
        "(locally\n  (declare (type (integer 0 10) x)\n           (optimize (speed 2) (safety 0))\n           (inline helper))\n  (the (integer 0 10) x))\n",
    );
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;
    let metadata = form
        .scope_metadata()
        .values()
        .find(|metadata| !metadata.declarations.is_empty())
        .ok_or_else(|| std::io::Error::other("declaration scope is missing"))?;
    let mut type_constant = None;
    let mut optimize_seen = false;
    let mut inline_seen = false;
    for declaration in &metadata.declarations {
        match declaration {
            HirDeclaration::Type {
                source_specifier, ..
            } => type_constant = Some(*source_specifier),
            HirDeclaration::Optimize { settings } => {
                optimize_seen = settings
                    .iter()
                    .any(|item| item.quality == HirQuality::Speed);
            }
            HirDeclaration::Inline { .. } => inline_seen = true,
            _ => {}
        }
    }
    let type_constant = type_constant
        .ok_or_else(|| std::io::Error::other("type declaration source constant is missing"))?;
    assert!(matches!(
        form.type_spec_constant(type_constant),
        Some(ncl_hir::Literal::Cons(_, _))
    ));
    assert!(optimize_seen && inline_seen);
    assert!(has_node(
        &form,
        |node| matches!(node, NodeKind::TypeAssertion { expected_type, .. } if form.type_spec_constant(*expected_type).is_some())
    ));
    assert_eq!(metadata.policy.speed().value(), 2);
    assert_eq!(metadata.policy.safety().value(), 0);
    lower_both_and_verify(&builder, &form, &expr)?;

    let foreign_id = ncl_hir::FormId::new(id.compilation_unit(), 1);
    let foreign_integer = fixture.intern("USER", "INTEGER");
    let value = fixture.form("THE", &[foreign_integer, Word::fixnum(7)]);
    let (_, foreign_form) = expand_build(
        &mut builder,
        &mut fixture,
        foreign_id,
        value,
        "(the user:integer 7)",
    )?;
    let asserted_type = foreign_form
        .nodes()
        .iter()
        .find_map(|(_, node)| match &node.kind {
            NodeKind::TypeAssertion { asserted_type, .. } => Some(*asserted_type),
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("type assertion is missing"))?;
    assert!(matches!(
        foreign_form
            .types()
            .get(asserted_type)
            .map(ncl_hir::ClType::kind),
        Some(ClTypeKind::Opaque { .. })
    ));
    Ok(())
}

#[test]
fn positive_safety_type_assertion_emits_checked_mir() -> TestResult<()> {
    let (id, mut builder, mut fixture) = context(44);
    let safety_name = fixture.cl("SAFETY");
    let safety = fixture.list(&[safety_name, Word::fixnum(3)]);
    let optimize_name = fixture.cl("OPTIMIZE");
    let optimize = fixture.list(&[optimize_name, safety]);
    let declaration = fixture.form("DECLARE", &[optimize]);
    let integer = fixture.cl("INTEGER");
    let integer_range = fixture.list(&[integer, Word::fixnum(0), Word::fixnum(10)]);
    let assertion = fixture.form("THE", &[integer_range, Word::fixnum(7)]);
    let input = fixture.form("LOCALLY", &[declaration, assertion]);
    let source = fixture_text(
        include_str!("../fixtures/hir-bridge/positive-safety.lisp"),
        "(locally\n  (declare (optimize (safety 3)))\n  (the (integer 0 10) 7))\n",
    );
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;
    let policy = form
        .scope_metadata()
        .values()
        .find(|metadata| metadata.policy.safety().value() == 3)
        .ok_or_else(|| std::io::Error::other("positive safety policy is missing"))?;
    assert_eq!(policy.policy.safety().value(), 3);

    let hir = ncl_hir::lower::lower(&form, builder.symbols())?;
    verify_ir(&hir.entry, "verify checked HIR MIR")?;
    assert!(hir.entry.blocks.iter().any(|block| {
        block.ops.iter().any(|op| {
            matches!(
                &op.kind,
                OpKind::Prim {
                    op: Prim::Typep,
                    ..
                }
            )
        })
    }));
    assert!(hir.entry.blocks.iter().any(|block| {
        block.ops.iter().any(|op| {
            matches!(
                &op.kind,
                OpKind::Builtin { name, .. } if name == "ncl_signal_type_error"
            )
        })
    }));
    assert!(
        hir.entry
            .blocks
            .iter()
            .any(|block| matches!(&block.terminator, Terminator::Throw { .. }))
    );

    let baseline = ncl_compiler_front::lower::lower_toplevel(&expr)?;
    verify_ir(&baseline.entry, "verify front baseline MIR")?;
    assert!(baseline.entry.blocks.iter().any(|block| {
        block.ops.iter().any(|op| {
            matches!(
                &op.kind,
                OpKind::TypeHint {
                    ty: ScalarTypeHint::Integer,
                    ..
                }
            )
        })
    }));
    assert!(!baseline.entry.blocks.iter().any(|block| {
        block.ops.iter().any(|op| {
            matches!(
                &op.kind,
                OpKind::Prim {
                    op: Prim::Typep,
                    ..
                }
            )
        })
    }));
    Ok(())
}

#[test]
fn local_declarations_preserve_binding_policy_and_unknown_arguments() -> TestResult<()> {
    // CLHS 3.8 defines these declaration specifiers and permits implementations to
    // ignore declaration names they do not recognize.
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_h.htm
    let (id, mut builder, mut fixture) = context(45);
    let special_name = fixture.user("*DYNAMIC*");
    let special_head = fixture.cl("SPECIAL");
    let special = fixture.list(&[special_head, special_name]);

    let function_type = fixture.cl("FUNCTION");
    let fixnum = fixture.cl("FIXNUM");
    let parameter_types = fixture.list(&[fixnum]);
    let function_specifier = fixture.list(&[function_type, parameter_types, fixnum]);
    let ftype_head = fixture.cl("FTYPE");
    let helper = fixture.user("HELPER");
    let ftype = fixture.list(&[ftype_head, function_specifier, helper]);

    let inline_head = fixture.cl("INLINE");
    let inline = fixture.list(&[inline_head, helper]);
    let notinline_head = fixture.cl("NOTINLINE");
    let slower = fixture.user("SLOWER");
    let notinline = fixture.list(&[notinline_head, slower]);
    let dynamic_extent_head = fixture.cl("DYNAMIC-EXTENT");
    let scratch = fixture.user("SCRATCH");
    let dynamic_extent = fixture.list(&[dynamic_extent_head, scratch]);
    let ignore_head = fixture.cl("IGNORE");
    let ignored = fixture.user("IGNORED");
    let ignore = fixture.list(&[ignore_head, ignored]);
    let ignorable_head = fixture.cl("IGNORABLE");
    let maybe_unused = fixture.user("MAYBE-UNUSED");
    let ignorable = fixture.list(&[ignorable_head, maybe_unused]);

    let speed = fixture.cl("SPEED");
    let space_name = fixture.cl("SPACE");
    let space_value = ncl_object::Word::fixnum(2);
    let space = fixture.list(&[space_name, space_value]);
    let debug_name = fixture.cl("DEBUG");
    let debug_value = ncl_object::Word::fixnum(1);
    let debug = fixture.list(&[debug_name, debug_value]);
    let compilation_speed_name = fixture.cl("COMPILATION-SPEED");
    let compilation_speed_value = ncl_object::Word::fixnum(3);
    let compilation_speed = fixture.list(&[compilation_speed_name, compilation_speed_value]);
    let optimize_head = fixture.cl("OPTIMIZE");
    let optimize = fixture.list(&[optimize_head, speed, space, debug, compilation_speed]);

    let extension_name = fixture.intern("USER", "VENDOR-NOTE");
    let extension_value = ncl_object::Word::fixnum(42);
    let extension_tag = fixture.user(":TAG");
    let unknown = fixture.list(&[extension_name, extension_value, extension_tag]);

    let declaration = fixture.form(
        "DECLARE",
        &[
            special,
            ftype,
            inline,
            notinline,
            dynamic_extent,
            ignore,
            ignorable,
            optimize,
            unknown,
        ],
    );
    let body = ncl_object::Word::fixnum(7);
    let input = fixture.form("LOCALLY", &[declaration, body]);
    let source = "(locally (declare (special *dynamic*) (ftype (function (fixnum) fixnum) helper) (inline helper) (notinline slower) (dynamic-extent scratch) (ignore ignored) (ignorable maybe-unused) (optimize speed (space 2) (debug 1) (compilation-speed 3)) (vendor-note 42 :tag)) 7)";
    let (expr, form) = expand_build(&mut builder, &mut fixture, id, input, source)?;

    let metadata = form
        .scope_metadata()
        .values()
        .find(|metadata| metadata.declarations.len() == 9)
        .ok_or_else(|| std::io::Error::other("all declaration forms should be retained"))?;
    assert!(metadata.declarations.iter().any(|declaration| {
        matches!(declaration, HirDeclaration::Special { names } if names.len() == 1)
    }));
    assert!(metadata.declarations.iter().any(|declaration| {
        matches!(declaration, HirDeclaration::Ftype { type_id, source_specifier, .. }
            if form.types().get(*type_id).is_some() && form.type_spec_constant(*source_specifier).is_some())
    }));
    assert!(metadata.declarations.iter().any(|declaration| {
        matches!(declaration, HirDeclaration::Inline { names } if names.len() == 1)
    }));
    assert!(metadata.declarations.iter().any(|declaration| {
        matches!(declaration, HirDeclaration::Notinline { names } if names.len() == 1)
    }));
    assert!(metadata.declarations.iter().any(|declaration| {
        matches!(declaration, HirDeclaration::DynamicExtent { names } if names.len() == 1)
    }));
    assert!(metadata.declarations.iter().any(|declaration| {
        matches!(declaration, HirDeclaration::Ignore { names } if names.len() == 1)
    }));
    assert!(metadata.declarations.iter().any(|declaration| {
        matches!(declaration, HirDeclaration::Ignorable { names } if names.len() == 1)
    }));
    assert!(metadata.declarations.iter().any(|declaration| {
        matches!(declaration, HirDeclaration::Optimize { settings }
            if settings.iter().any(|setting| setting.quality == HirQuality::CompilationSpeed && setting.value == 3))
    }));
    assert!(metadata.declarations.iter().any(|declaration| {
        matches!(declaration, HirDeclaration::Unknown { arguments, .. }
            if arguments.len() == 2 && matches!(arguments[0], ncl_hir::Literal::Number(ncl_hir::NumberLiteral::Fixnum(42))))
    }));
    assert_eq!(metadata.policy.speed().value(), 3);
    assert_eq!(metadata.policy.space().value(), 2);
    assert_eq!(metadata.policy.debug().value(), 1);
    let _ = lower_both_and_verify(&builder, &form, &expr)?;
    Ok(())
}
