use super::{TestResult, constant_for_value, context, returned_value};
use ncl_hir::binding::BindingUseKind;
use ncl_hir::{
    HirForm, LambdaKey, LambdaOptional, LambdaSignature, Literal, NodeKind, NumberLiteral,
    SymbolTable,
};
use ncl_ir::{Constant, Function, OpKind, Terminator};

fn value_for_constant(function: &Function, expected: &Constant) -> Option<ncl_ir::ValueId> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|operation| &operation.results)
        .filter_map(|(value, _)| {
            (constant_for_value(function, *value) == Some(expected)).then_some(*value)
        })
        .next_back()
}

fn build_defaulted_lambda() -> TestResult<(HirForm, SymbolTable)> {
    let source =
        "(lambda (&optional (limit 31 limit-p)) limit) (lambda (&key (count 59 count-p)) count)";
    let (form_id, mut builder, mut symbols) = context(10, source)?;
    let definition_scope = builder.root_scope();
    let optional_scope = builder.bindings_mut().push_scope(definition_scope)?;
    let optional_name = symbols.intern(form_id, Some("COMMON-LISP-USER"), "LIMIT", None)?;
    let optional_supplied_name =
        symbols.intern(form_id, Some("COMMON-LISP-USER"), "LIMIT-P", None)?;
    builder.register_symbol(optional_name)?;
    builder.register_symbol(optional_supplied_name)?;
    let optional_binding = builder
        .bindings_mut()
        .bind_variable(optional_scope, optional_name)?;
    let optional_supplied = builder
        .bindings_mut()
        .bind_variable(optional_scope, optional_supplied_name)?;
    let key_scope = builder.bindings_mut().push_scope(optional_scope)?;
    let key_name = symbols.intern(form_id, Some("COMMON-LISP-USER"), "COUNT", None)?;
    let key_supplied_name = symbols.intern(form_id, Some("COMMON-LISP-USER"), "COUNT-P", None)?;
    builder.register_symbol(key_name)?;
    builder.register_symbol(key_supplied_name)?;
    let key_binding = builder.bindings_mut().bind_variable(key_scope, key_name)?;
    let key_supplied = builder
        .bindings_mut()
        .bind_variable(key_scope, key_supplied_name)?;
    let keyword = symbols.intern(form_id, Some("KEYWORD"), "COUNT", None)?;
    builder.register_symbol(keyword)?;
    let optional_default = builder.alloc_node(
        definition_scope,
        NodeKind::Literal(Literal::Number(NumberLiteral::Fixnum(31))),
    )?;
    let key_default = builder.alloc_node(
        optional_scope,
        NodeKind::Literal(Literal::Number(NumberLiteral::Fixnum(59))),
    )?;
    let key_reference = builder.alloc_node(
        key_scope,
        NodeKind::VariableReference {
            target: ncl_hir::VariableTarget::Lexical(key_binding),
        },
    )?;
    builder
        .bindings_mut()
        .record_use(key_binding, key_reference, BindingUseKind::Read)?;
    let lambda = builder.alloc_node(
        definition_scope,
        NodeKind::Lambda {
            parameters: vec![key_binding, key_supplied],
            body_scope: key_scope,
            body: vec![key_reference],
        },
    )?;
    builder.set_lambda_signature_for_lambda(
        lambda,
        LambdaSignature {
            required: Vec::new(),
            optional: vec![LambdaOptional {
                binding: optional_binding,
                default: Some(optional_default),
                initializer_scope: definition_scope,
                supplied_p: Some(optional_supplied),
            }],
            rest: None,
            keys: vec![LambdaKey {
                keyword,
                binding: key_binding,
                default: Some(key_default),
                initializer_scope: optional_scope,
                supplied_p: Some(key_supplied),
            }],
            allow_other_keys: false,
            aux: Vec::new(),
        },
    )?;
    builder.add_root(lambda)?;
    Ok((builder.finish()?, symbols))
}

fn assert_optional_default_values(lambda: &Function) -> TestResult<()> {
    let default_value = value_for_constant(lambda, &Constant::Fixnum(31))
        .ok_or_else(|| std::io::Error::other("optional default value is missing"))?;
    let default_target = lambda
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            Terminator::Jump { target, args }
                if args.first() == Some(&default_value)
                    && args
                        .get(1)
                        .and_then(|value| constant_for_value(lambda, *value))
                        == Some(&Constant::Nil) =>
            {
                Some(*target)
            }
            _ => None,
        });
    assert!(
        default_target.is_some(),
        "omitted optional uses its default and NIL supplied-p"
    );
    assert!(lambda.blocks.iter().any(|block| {
        block
            .ops
            .iter()
            .any(|operation| matches!(&operation.kind, OpKind::LoadArg { index: 1 }))
    }));
    assert!(lambda.blocks.iter().any(|block| {
        matches!(&block.terminator, Terminator::Jump { args, .. }
            if args.get(1).and_then(|value| constant_for_value(lambda, *value)) == Some(&Constant::T))
    }), "a supplied optional argument sets its supplied-p flag");
    Ok(())
}

fn assert_keyword_default_values(lambda: &Function) -> TestResult<()> {
    let default_value = value_for_constant(lambda, &Constant::Fixnum(59))
        .ok_or_else(|| std::io::Error::other("keyword default value is missing"))?;
    let default_target = lambda
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            Terminator::Jump { target, args } if args.first() == Some(&default_value) => {
                Some(*target)
            }
            _ => None,
        });
    assert!(
        default_target.is_some(),
        "omitted keyword uses its declared default"
    );
    let returned = returned_value(lambda)
        .ok_or_else(|| std::io::Error::other("keyword value is not returned"))?;
    let merged_value = lambda
        .blocks
        .iter()
        .find(|block| Some(block.id) == default_target)
        .and_then(|block| block.params.first())
        .map(|parameter| parameter.value);
    assert_eq!(merged_value, Some(returned));
    for builtin in [
        "make-rest-list",
        "check-keywords",
        "keyword-value",
        "keyword-supplied-p",
    ] {
        assert!(
            lambda
                .blocks
                .iter()
                .flat_map(|block| &block.ops)
                .any(|operation| {
                    matches!(&operation.kind, OpKind::Builtin { name, .. } if name == builtin)
                }),
            "keyword prologue emits {builtin}"
        );
    }
    assert!(lambda.constants.contains(&Constant::Symbol {
        package: "KEYWORD".to_owned(),
        name: "COUNT".to_owned(),
    }));
    Ok(())
}

#[test]
fn optional_and_keyword_lambda_defaults_reach_their_result_values() -> TestResult<()> {
    let (form, symbols) = build_defaulted_lambda()?;
    let lowered = ncl_hir::lower::lower(&form, &symbols)?;
    let lambda = lowered
        .nested
        .first()
        .ok_or_else(|| std::io::Error::other("lambda function is missing"))?;
    assert_optional_default_values(lambda)?;
    assert_keyword_default_values(lambda)?;
    // CLHS ordinary lambda lists evaluate defaults for omitted optional and keyword arguments.
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_da.htm
    Ok(())
}
