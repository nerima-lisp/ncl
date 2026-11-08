use crate::forms::Fixture;
use ncl_compiler_front::hir_builder::{HirBuildError, HirBuilder, UnsupportedHirForm};
use ncl_compiler_front::{Declaration, Expr, Literal, OptimizeQuality, Quality};
use ncl_hir::{CompilationUnitId, FormId};

use super::common::{TestResult, fixture_text, source_origin};

#[test]
fn retained_local_macro_definitions_and_unknown_quality_fail_typed() -> TestResult<()> {
    let unit = CompilationUnitId::new(41);
    let mut builder = HirBuilder::new(unit);
    let mut fixture = Fixture::new();
    let name = fixture.user("M");
    let parameter = fixture.user("X");
    let lambda_list = fixture.list(&[parameter]);
    let quoted_body = fixture.form("QUOTE", &[parameter]);
    let definition = fixture.list(&[name, lambda_list, quoted_body]);
    let definitions = fixture.list(&[definition]);
    let macrolet = fixture.form("MACROLET", &[definitions]);
    let expr = fixture.expand(macrolet)?;
    let source = fixture_text(
        include_str!("../fixtures/hir-bridge/unsupported.lisp"),
        "(macrolet ((m (x) (quote x)))\n  (m 1))\n(symbol-macrolet ((x y))\n  x)\n",
    );
    let id = FormId::new(unit, 0);
    let (origins, origin) = source_origin(id, source)?;
    assert!(matches!(
        builder.build_form(id, &expr, origins, origin),
        Err(HirBuildError::Unrepresentable(UnsupportedHirForm::Macrolet))
    ));

    let name = fixture.user("X");
    let expansion = fixture.user("Y");
    let definition = fixture.list(&[name, expansion]);
    let definitions = fixture.list(&[definition]);
    let body = fixture.user("X");
    let symbol_macrolet = fixture.form("SYMBOL-MACROLET", &[definitions, body]);
    let expr = fixture.expand(symbol_macrolet)?;
    let id = FormId::new(unit, 1);
    let (origins, origin) = source_origin(id, source)?;
    assert!(matches!(
        builder.build_form(id, &expr, origins, origin),
        Err(HirBuildError::Unrepresentable(
            UnsupportedHirForm::SymbolMacrolet
        ))
    ));

    let id = FormId::new(unit, 2);
    let expr = Expr::Locally {
        declarations: vec![Declaration::Optimize(vec![OptimizeQuality {
            quality: Quality::Unknown,
            value: 1,
        }])],
        body: vec![Expr::Constant(Literal::Nil)],
    };
    let (origins, origin) = source_origin(id, "(locally (declare (optimize (unknown 1))) nil)")?;
    assert!(matches!(
        builder.build_form(id, &expr, origins, origin),
        Err(HirBuildError::Unrepresentable(
            UnsupportedHirForm::UnknownOptimizationQuality
        ))
    ));
    Ok(())
}
