use std::error::Error;

use super::{form_builder, symbol_table};
use ncl_hir::cl_type::{
    ArrayDimensions, ArrayType, BuiltinType, FunctionParameters, FunctionType, MemberValue,
    OpaqueOperand,
};
use ncl_hir::{
    CompilationUnitId, FormId, HirDeclaration, HirQuality, Literal, OptimizationPolicy,
    OptimizationSetting, ScopeMetadata, SymbolId,
};

#[test]
fn composite_type_operands_remain_owned_and_verifiable() -> Result<(), Box<dyn Error>> {
    let form = FormId::new(CompilationUnitId::new(40), 0);
    let specifier = SymbolId::try_from_index(form.compilation_unit(), 0)?;
    let class = SymbolId::try_from_index(form.compilation_unit(), 1)?;
    let member_symbol = SymbolId::try_from_index(form.compilation_unit(), 2)?;
    let source_symbol = SymbolId::try_from_index(form.compilation_unit(), 3)?;
    let mut builder = form_builder(form)?;
    let constant = builder.add_type_spec_constant(Literal::Symbol(source_symbol))?;
    let types = builder.types_mut();
    let integer = types.named(BuiltinType::Integer)?;
    let member = types.member(vec![
        MemberValue::Integer(17),
        MemberValue::Symbol(member_symbol),
    ])?;
    let union = types.union([integer, member])?;
    let intersection = types.intersection([union, integer])?;
    let complement = types.complement(intersection)?;
    let cons = types.cons(integer, complement)?;
    let array = types.array(ArrayType {
        element_type: Some(cons),
        dimensions: ArrayDimensions::Axes(vec![Some(1), None]),
        simple: true,
    })?;
    let fixed_function = types.function(FunctionType {
        parameters: FunctionParameters::Fixed(vec![array, integer]),
        returns: cons,
    })?;
    let values = types.multiple_values([fixed_function, member])?;
    let class_type = types.class(class)?;
    let opaque = types.opaque(
        specifier,
        [
            OpaqueOperand::Type(values),
            OpaqueOperand::Constant(constant),
            OpaqueOperand::Symbol(source_symbol),
        ],
    )?;
    let _ = types.function(FunctionType {
        parameters: FunctionParameters::Any,
        returns: class_type,
    })?;
    let _ = types.array(ArrayType {
        element_type: None,
        dimensions: ArrayDimensions::Any,
        simple: false,
    })?;

    let hir = builder.finish()?;
    assert!(hir.types().get(opaque).is_some());
    assert_eq!(
        hir.verify(&symbol_table(form, &["TYPE", "CLASS", "MEMBER", "SOURCE"])?),
        Ok(())
    );
    Ok(())
}

#[test]
fn declaration_variants_preserve_symbols_types_and_opaque_operands() -> Result<(), Box<dyn Error>> {
    let unit = CompilationUnitId::new(46);
    let form = FormId::new(unit, 0);
    let mut builder = form_builder(form)?;
    let root = builder.root_scope();
    let special = SymbolId::try_from_index(unit, 0)?;
    let inline = SymbolId::try_from_index(unit, 1)?;
    let notinline = SymbolId::try_from_index(unit, 2)?;
    let unknown_quality = SymbolId::try_from_index(unit, 3)?;
    let dynamic = SymbolId::try_from_index(unit, 4)?;
    let ignored = SymbolId::try_from_index(unit, 5)?;
    let ignorable = SymbolId::try_from_index(unit, 6)?;
    let unknown_name = SymbolId::try_from_index(unit, 7)?;
    let operand_a = SymbolId::try_from_index(unit, 8)?;
    let operand_b = SymbolId::try_from_index(unit, 9)?;
    let declarations = vec![
        HirDeclaration::Special {
            names: vec![special],
        },
        HirDeclaration::Inline {
            names: vec![inline],
        },
        HirDeclaration::Notinline {
            names: vec![notinline],
        },
        HirDeclaration::Optimize {
            settings: vec![
                OptimizationSetting {
                    quality: HirQuality::Speed,
                    value: 3,
                },
                OptimizationSetting {
                    quality: HirQuality::Unknown(unknown_quality),
                    value: 2,
                },
            ],
        },
        HirDeclaration::DynamicExtent {
            names: vec![dynamic],
        },
        HirDeclaration::Ignore {
            names: vec![ignored],
        },
        HirDeclaration::Ignorable {
            names: vec![ignorable],
        },
        HirDeclaration::Unknown {
            name: unknown_name,
            arguments: vec![Literal::Cons(
                Box::new(Literal::Symbol(operand_a)),
                Box::new(Literal::Vector(vec![Literal::Symbol(operand_b)])),
            )],
        },
    ];
    builder.set_scope_metadata(
        root,
        ScopeMetadata {
            declarations: declarations.clone(),
            policy: OptimizationPolicy::default(),
        },
    )?;
    let hir = builder.finish()?;
    let metadata = hir
        .scope_metadata()
        .get(&root)
        .ok_or("root declarations were not retained")?;
    assert_eq!(metadata.declarations, declarations);
    assert_eq!(
        hir.verify(&symbol_table(
            form,
            &[
                "SPECIAL",
                "INLINE",
                "NOTINLINE",
                "QUALITY",
                "DYNAMIC",
                "IGNORE",
                "IGNORABLE",
                "DECLARATION",
                "CAR",
                "CDR",
            ],
        )?),
        Ok(())
    );
    Ok(())
}
