use ncl_compiler_front::hir_builder::{HirBuildError, UnsupportedHirForm};
use ncl_hir::{
    BindingError, ClTypeError, ControlError, FormError, IdError, NodeKind,
    OptimizationQualityError, SymbolError,
};

use super::common::{context, source_origin};

#[test]
fn return_from_without_a_visible_block_reports_unknown_block()
-> Result<(), Box<dyn std::error::Error>> {
    let (id, mut builder, mut fixture) = context(77);
    let name = fixture.user("MISSING-BLOCK");
    let input = fixture.form("RETURN-FROM", &[name, ncl_object::Word::fixnum(7)]);
    let expr = fixture.expand(input)?;
    let source = "(return-from missing-block 7)";
    let (origins, origin) = source_origin(id, source)?;

    assert!(matches!(
        builder.build_form(id, &expr, origins, origin),
        Err(HirBuildError::Unrepresentable(
            UnsupportedHirForm::UnknownBlock
        ))
    ));
    // CLHS BLOCK gives RETURN-FROM a lexical target only within the block's body.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_block.htm
    Ok(())
}

#[test]
fn go_without_a_visible_tagbody_tag_reports_unknown_tag() -> Result<(), Box<dyn std::error::Error>>
{
    let (id, mut builder, mut fixture) = context(78);
    let tag = fixture.user("MISSING-TAG");
    let input = fixture.form("GO", &[tag]);
    let expr = fixture.expand(input)?;
    let source = "(go missing-tag)";
    let (origins, origin) = source_origin(id, source)?;

    assert!(matches!(
        builder.build_form(id, &expr, origins, origin),
        Err(HirBuildError::Unrepresentable(
            UnsupportedHirForm::UnknownTag
        ))
    ));
    // CLHS GO resolves a tag in a lexically containing TAGBODY.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_go.htm
    Ok(())
}

#[test]
fn flet_return_from_keeps_its_enclosing_block_target_in_hir()
-> Result<(), Box<dyn std::error::Error>> {
    let (id, mut builder, mut fixture) = context(80);
    let block_name = fixture.user("DONE");
    let function_name = fixture.user("LEAVE");
    let lambda_list = fixture.list(&[]);
    let return_from = fixture.form("RETURN-FROM", &[block_name, ncl_object::Word::fixnum(23)]);
    let definition = fixture.list(&[function_name, lambda_list, return_from]);
    let definitions = fixture.list(&[definition]);
    let call = fixture.form("LEAVE", &[]);
    let flet = fixture.form("FLET", &[definitions, call]);
    let input = fixture.form("BLOCK", &[block_name, flet, ncl_object::Word::fixnum(29)]);
    let expr = fixture.expand(input)?;
    let source = "(block done (flet ((leave () (return-from done 23))) (leave) 29))";
    let (origins, origin) = source_origin(id, source)?;
    let form = builder.build_form(id, &expr, origins, origin)?;

    let block = form
        .nodes()
        .iter()
        .find_map(|(index, node)| {
            if matches!(&node.kind, NodeKind::Block { .. }) {
                ncl_hir::NodeId::try_from_index(id, index).ok()
            } else {
                None
            }
        })
        .ok_or_else(|| std::io::Error::other("enclosing BLOCK node is missing"))?;
    let value = form
        .nodes()
        .iter()
        .find_map(|(_, node)| match &node.kind {
            NodeKind::ReturnFrom {
                target,
                value: Some(value),
            } if *target == block => Some(*value),
            _ => None,
        })
        .ok_or_else(|| std::io::Error::other("FLET RETURN-FROM target is missing"))?;
    assert!(matches!(
        form.nodes().get(value).map(|node| &node.kind),
        Some(NodeKind::Literal(ncl_hir::Literal::Number(
            ncl_hir::NumberLiteral::Fixnum(23)
        )))
    ));
    form.verify(builder.symbols())?;
    // CLHS BLOCK permits RETURN-FROM from an invoked FLET function in its dynamic extent.
    // https://www.lispworks.com/documentation/HyperSpec/Body/s_block.htm
    Ok(())
}

#[test]
fn source_errors_preserve_their_layer_when_mapped_to_the_front_bridge()
-> Result<(), Box<dyn std::error::Error>> {
    let index = usize::try_from(u64::from(u32::MAX) + 1)?;
    let id_error = IdError::IndexOverflow { index };

    assert_eq!(
        HirBuildError::from(FormError::InvalidFunctionSignature),
        HirBuildError::Form(FormError::InvalidFunctionSignature)
    );
    assert_eq!(
        HirBuildError::from(SymbolError::InvalidIdentityShape),
        HirBuildError::Symbol(SymbolError::InvalidIdentityShape)
    );
    assert_eq!(
        HirBuildError::from(BindingError::Id(id_error)),
        HirBuildError::Binding(BindingError::Id(id_error))
    );
    assert_eq!(
        HirBuildError::from(ControlError::Id(id_error)),
        HirBuildError::Control(ControlError::Id(id_error))
    );
    assert_eq!(
        HirBuildError::from(ClTypeError::Id(id_error)),
        HirBuildError::Type(ClTypeError::Id(id_error))
    );
    assert_eq!(
        HirBuildError::from(OptimizationQualityError::OutOfRange(4)),
        HirBuildError::Policy(OptimizationQualityError::OutOfRange(4))
    );
    assert_eq!(HirBuildError::from(id_error), HirBuildError::Id(id_error));

    let displayed = HirBuildError::Unrepresentable(UnsupportedHirForm::UnknownTag);
    assert_eq!(
        displayed.to_string(),
        "front-to-HIR bridge failed: Unrepresentable(UnknownTag)"
    );
    Ok(())
}
