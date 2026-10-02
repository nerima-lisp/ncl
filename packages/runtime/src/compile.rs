// This module's declaration-driven optimization-level resolution is public
// API for a future caller (selecting a pipeline from `(optimize ...)`
// declarations) that has not landed yet; nothing in this crate calls it yet.
#![allow(dead_code)]

use std::path::Path;

use crate::{Runtime, RuntimeError};
use ncl_compiler_front::{Declaration, Expr, FunctionDesignator, LambdaExpr, Operator, Quality};
use ncl_object::Word;
use ncl_object::{Runtime as ObjectRuntime, ThreadContext};
use ncl_reader::{ReadOptions, StringSource, read};

const FASL_FEATURES: u64 = 0;
const PAYLOAD_MAGIC: &[u8] = b"NCLRTFASL";
const PAYLOAD_VERSION: u16 = 1;

/// The optimization policy selected for one expanded form.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptimizationLevel {
    /// Prefer the existing safe optimization pipeline for execution speed.
    Speed,
    /// Keep the default safety-first pipeline.
    Safety,
    /// Preserve the unoptimized IR for better debugging behavior.
    Debug,
}

#[derive(Clone, Copy, Debug)]
struct OptimizationProfile {
    speed: u8,
    safety: u8,
    debug: u8,
    unsupported: bool,
}

impl Default for OptimizationProfile {
    fn default() -> Self {
        Self {
            speed: 1,
            safety: 3,
            debug: 1,
            unsupported: false,
        }
    }
}

impl OptimizationProfile {
    fn apply(&mut self, declarations: &[Declaration]) {
        for declaration in declarations {
            let Declaration::Optimize(qualities) = declaration else {
                continue;
            };
            for quality in qualities {
                match quality.quality {
                    Quality::Speed => self.speed = quality.value,
                    Quality::Safety => self.safety = quality.value,
                    Quality::Debug => self.debug = quality.value,
                    Quality::Space | Quality::CompilationSpeed | Quality::Unknown => {
                        self.unsupported = true;
                    }
                    _ => self.unsupported = true,
                }
            }
        }
    }

    const fn level(self) -> OptimizationLevel {
        if self.unsupported {
            return OptimizationLevel::Safety;
        }
        if self.debug >= self.speed && self.debug >= self.safety && self.debug > 1 {
            return OptimizationLevel::Debug;
        }
        if self.speed > self.safety {
            return OptimizationLevel::Speed;
        }
        OptimizationLevel::Safety
    }
}

/// Resolve optimize declarations in an expanded form without making unknown
/// or unsupported declarations increase optimization beyond the safe default.
pub fn optimization_level(expr: &Expr) -> OptimizationLevel {
    let mut profile = OptimizationProfile::default();
    visit_expr(expr, &mut profile);
    profile.level()
}

fn visit_declarations(declarations: &[Declaration], profile: &mut OptimizationProfile) {
    profile.apply(declarations);
}

fn visit_lambda(lambda: &LambdaExpr, profile: &mut OptimizationProfile) {
    visit_declarations(&lambda.declarations, profile);
    for form in &lambda.body {
        visit_expr(form, profile);
    }
}

fn visit_operator(operator: &Operator, profile: &mut OptimizationProfile) {
    if let Operator::Lambda(lambda) = operator {
        visit_lambda(lambda, profile);
    }
}

fn visit_function(function: &FunctionDesignator, profile: &mut OptimizationProfile) {
    if let FunctionDesignator::Lambda(lambda) = function {
        visit_lambda(lambda, profile);
    }
}

#[allow(clippy::too_many_lines)]
fn visit_expr(expr: &Expr, profile: &mut OptimizationProfile) {
    match expr {
        Expr::Call {
            operator,
            arguments,
        } => {
            visit_operator(operator, profile);
            for argument in arguments {
                visit_expr(argument, profile);
            }
        }
        Expr::Function(function) => visit_function(function, profile),
        Expr::Lambda(lambda) => visit_lambda(lambda, profile),
        Expr::If {
            test,
            then,
            otherwise,
        } => {
            visit_expr(test, profile);
            visit_expr(then, profile);
            if let Some(otherwise) = otherwise {
                visit_expr(otherwise, profile);
            }
        }
        Expr::Progn(forms)
        | Expr::Block { body: forms, .. }
        | Expr::EvalWhen { body: forms, .. }
        | Expr::Locally { body: forms, .. } => {
            if let Expr::Locally { declarations, .. } = expr {
                visit_declarations(declarations, profile);
            }
            for form in forms {
                visit_expr(form, profile);
            }
        }
        Expr::ReturnFrom {
            value: Some(value), ..
        } => visit_expr(value, profile),
        Expr::Tagbody(items) => {
            for item in items {
                if let ncl_compiler_front::TagbodyItem::Form(form) = item {
                    visit_expr(form, profile);
                }
            }
        }
        Expr::Catch { tag, body } => {
            visit_expr(tag, profile);
            for form in body {
                visit_expr(form, profile);
            }
        }
        Expr::Throw { tag, value } => {
            visit_expr(tag, profile);
            visit_expr(value, profile);
        }
        Expr::UnwindProtect { protected, cleanup } => {
            visit_expr(protected, profile);
            for form in cleanup {
                visit_expr(form, profile);
            }
        }
        Expr::Let {
            bindings,
            declarations,
            body,
            ..
        } => {
            visit_declarations(declarations, profile);
            for binding in bindings {
                if let Some(value) = &binding.value {
                    visit_expr(value, profile);
                }
            }
            for form in body {
                visit_expr(form, profile);
            }
        }
        Expr::Progv {
            symbols,
            values,
            body,
        } => {
            visit_expr(symbols, profile);
            visit_expr(values, profile);
            for form in body {
                visit_expr(form, profile);
            }
        }
        Expr::Setq(assignments) => {
            for (_, value) in assignments {
                visit_expr(value, profile);
            }
        }
        Expr::MultipleValueCall {
            function,
            arguments,
        } => {
            visit_expr(function, profile);
            for argument in arguments {
                visit_expr(argument, profile);
            }
        }
        Expr::MultipleValueProg1 { first, forms } => {
            visit_expr(first, profile);
            for form in forms {
                visit_expr(form, profile);
            }
        }
        Expr::The { value, .. } | Expr::LoadTimeValue { form: value, .. } => {
            visit_expr(value, profile);
        }
        Expr::Flet {
            definitions,
            declarations,
            body,
        }
        | Expr::Labels {
            definitions,
            declarations,
            body,
        } => {
            visit_declarations(declarations, profile);
            for definition in definitions {
                visit_lambda(&definition.lambda, profile);
            }
            for form in body {
                visit_expr(form, profile);
            }
        }
        Expr::Macrolet {
            definitions,
            declarations,
            body,
        } => {
            visit_declarations(declarations, profile);
            for definition in definitions {
                visit_declarations(&definition.declarations, profile);
                for form in &definition.body {
                    visit_expr(form, profile);
                }
            }
            for form in body {
                visit_expr(form, profile);
            }
        }
        Expr::SymbolMacrolet {
            declarations,
            body,
            definitions,
        } => {
            visit_declarations(declarations, profile);
            for definition in definitions {
                visit_expr(&definition.expansion, profile);
            }
            for form in body {
                visit_expr(form, profile);
            }
        }
        _ => profile.unsupported = true,
    }
}

pub fn source(runtime: &mut Runtime, source: &str) -> Result<Word, RuntimeError> {
    runtime.eval(source)
}

pub fn file(runtime: &mut Runtime, path: &Path) -> Result<Word, RuntimeError> {
    let bytes = std::fs::read(path).map_err(|error| RuntimeError::Io {
        path: path.display().to_string(),
        error,
    })?;
    let contents = String::from_utf8(bytes)
        .map_err(|_| RuntimeError::Native("source file is not valid UTF-8".to_owned()))?;
    let value = crate::load::source_forms_with_mode(
        runtime,
        &contents,
        crate::evalwhen::TopLevelMode::CompileFile,
    )?;
    // This first FASL format deliberately stores source, so load recompiles it.
    // Native code, constants, and fixups are reserved for the persistent FASL format.
    let fasl = ncl_objfile::Fasl {
        header: ncl_objfile::FaslHeader {
            architecture: host_architecture(),
            features: FASL_FEATURES,
        },
        sections: ncl_objfile::FaslSection {
            code: Vec::new(),
            relocations: Vec::new(),
            constants: Vec::new(),
            symbols: Vec::new(),
            stack_maps: Vec::new(),
            debug: encode_payload(contents.as_bytes())?,
        },
    };
    let bytes = ncl_objfile::FaslWriter::write(&fasl)?;
    let output_path = path.with_extension("fasl");
    std::fs::write(&output_path, bytes).map_err(|error| RuntimeError::Io {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(value)
}

pub fn is_fasl(bytes: &[u8]) -> bool {
    bytes.get(..8) == Some(b"NCLFASL\0")
}

pub fn decode_payload(bytes: &[u8]) -> Result<String, RuntimeError> {
    let header_len = PAYLOAD_MAGIC.len() + 2 + 8 + 8;
    if bytes.len() < header_len || bytes.get(..PAYLOAD_MAGIC.len()) != Some(PAYLOAD_MAGIC) {
        return Err(RuntimeError::Native(
            "invalid runtime FASL payload magic".to_owned(),
        ));
    }
    let version_start = PAYLOAD_MAGIC.len();
    let version = u16::from_le_bytes(
        bytes
            .get(version_start..version_start + 2)
            .ok_or_else(|| RuntimeError::Native("invalid runtime FASL payload version".to_owned()))?
            .try_into()
            .map_err(|_| RuntimeError::Native("invalid runtime FASL payload version".to_owned()))?,
    );
    if version != PAYLOAD_VERSION {
        return Err(RuntimeError::Native(
            "unsupported runtime FASL payload version".to_owned(),
        ));
    }
    let length_start = version_start + 2;
    let source_len = u64::from_le_bytes(
        bytes
            .get(length_start..length_start + 8)
            .ok_or_else(|| RuntimeError::Native("invalid runtime FASL source length".to_owned()))?
            .try_into()
            .map_err(|_| RuntimeError::Native("invalid runtime FASL source length".to_owned()))?,
    );
    let hash_start = length_start + 8;
    let expected_hash = u64::from_le_bytes(
        bytes
            .get(hash_start..hash_start + 8)
            .ok_or_else(|| RuntimeError::Native("invalid runtime FASL source hash".to_owned()))?
            .try_into()
            .map_err(|_| RuntimeError::Native("invalid runtime FASL source hash".to_owned()))?,
    );
    let source_start = header_len;
    let source_end = source_start
        .checked_add(
            usize::try_from(source_len)
                .map_err(|_| RuntimeError::Native("runtime FASL source is too large".to_owned()))?,
        )
        .ok_or_else(|| RuntimeError::Native("runtime FASL source length overflow".to_owned()))?;
    if source_end != bytes.len() {
        return Err(RuntimeError::Native(
            "invalid runtime FASL payload length".to_owned(),
        ));
    }
    let source = bytes
        .get(source_start..source_end)
        .ok_or_else(|| RuntimeError::Native("invalid runtime FASL source range".to_owned()))?;
    if fnv1a(source) != expected_hash {
        return Err(RuntimeError::Native(
            "runtime FASL source hash mismatch".to_owned(),
        ));
    }
    String::from_utf8(source.to_vec())
        .map_err(|_| RuntimeError::Native("runtime FASL source is not valid UTF-8".to_owned()))
}

fn encode_payload(source: &[u8]) -> Result<Vec<u8>, RuntimeError> {
    let source_len = u64::try_from(source.len())
        .map_err(|_| RuntimeError::Native("source is too large for runtime FASL".to_owned()))?;
    let mut payload =
        Vec::with_capacity(PAYLOAD_MAGIC.len() + 2 + std::mem::size_of::<u64>() * 2 + source.len());
    payload.extend_from_slice(PAYLOAD_MAGIC);
    payload.extend_from_slice(&PAYLOAD_VERSION.to_le_bytes());
    payload.extend_from_slice(&source_len.to_le_bytes());
    payload.extend_from_slice(&fnv1a(source).to_le_bytes());
    payload.extend_from_slice(source);
    Ok(payload)
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

const fn host_architecture() -> ncl_objfile::Architecture {
    if cfg!(target_arch = "x86_64") {
        ncl_objfile::Architecture::X86_64
    } else {
        ncl_objfile::Architecture::Aarch64
    }
}

pub fn read_forms(
    context: &mut ThreadContext,
    object: &ObjectRuntime,
    source: &str,
) -> Result<Vec<Word>, RuntimeError> {
    let options = ReadOptions::standard(context, object)?;
    let mut input = StringSource::new(source);
    let mut forms = Vec::new();
    while let Some(form) = read(context, object, &mut input, &options)? {
        forms.push(form);
    }
    Ok(forms)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{decode_payload, encode_payload, is_fasl, optimization_level, read_forms};
    use ncl_compiler_front::ast::{
        Expr, FunctionDesignator, LambdaExpr, LetBinding, LocalFunction, LocalMacro, Operator,
        SymbolMacro, TagbodyItem,
    };
    use ncl_compiler_front::lambda_list::LambdaList;
    use ncl_compiler_front::literal::Literal;
    use ncl_compiler_front::{Declaration, OptimizeQuality, Quality, SymbolRef, TypeSpecifier};
    use ncl_object::{Runtime as ObjectRuntime, ThreadContext};

    fn optimize(quality: Quality, value: u8) -> Declaration {
        Declaration::Optimize(vec![OptimizeQuality { quality, value }])
    }

    fn probe() -> Expr {
        Expr::Locally {
            declarations: vec![optimize(Quality::Debug, 3)],
            body: Vec::new(),
        }
    }

    #[test]
    fn optimization_level_follows_nested_declarations_and_safe_fallbacks() {
        let speed = Declaration::Optimize(vec![OptimizeQuality {
            quality: Quality::Speed,
            value: 1,
        }]);
        let debug = Declaration::Optimize(vec![OptimizeQuality {
            quality: Quality::Debug,
            value: 2,
        }]);
        let safety = Declaration::Optimize(vec![OptimizeQuality {
            quality: Quality::Safety,
            value: 1,
        }]);
        let unsupported = Declaration::Optimize(vec![OptimizeQuality {
            quality: Quality::Space,
            value: 3,
        }]);

        let nested = Expr::Locally {
            declarations: vec![speed, safety, debug],
            body: Vec::new(),
        };
        assert_eq!(optimization_level(&nested), super::OptimizationLevel::Debug);

        let unsafe_request = Expr::Progn(vec![
            Expr::Locally {
                declarations: vec![unsupported],
                body: vec![Expr::Constant(ncl_compiler_front::literal::Literal::Nil)],
            },
            Expr::Constant(ncl_compiler_front::literal::Literal::Nil),
        ]);
        assert_eq!(
            optimization_level(&unsafe_request),
            super::OptimizationLevel::Safety
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn optimization_level_walks_all_supported_expression_shapes() {
        let lambda = LambdaExpr {
            lambda_list: LambdaList::new(),
            declarations: vec![optimize(Quality::Debug, 3)],
            docstring: None,
            body: vec![probe()],
        };
        let local_function = LocalFunction {
            name: SymbolRef::interned("COMMON-LISP-USER", "LOCAL"),
            lambda: lambda.clone(),
        };
        let local_macro = LocalMacro {
            name: SymbolRef::interned("COMMON-LISP-USER", "MACRO"),
            lambda_list: LambdaList::new(),
            declarations: vec![optimize(Quality::Debug, 3)],
            docstring: None,
            body: vec![probe()],
        };
        let symbol_macro = SymbolMacro {
            name: SymbolRef::interned("COMMON-LISP-USER", "SYMBOL"),
            expansion: probe(),
        };
        let name = SymbolRef::interned("COMMON-LISP-USER", "FUNCTION");
        let forms = vec![
            (
                "call with named operator",
                Expr::Call {
                    operator: Operator::Name(name.clone()),
                    arguments: vec![probe()],
                },
            ),
            (
                "call with lambda operator",
                Expr::Call {
                    operator: Operator::Lambda(Box::new(lambda.clone())),
                    arguments: vec![probe()],
                },
            ),
            (
                "named function designator",
                Expr::Function(FunctionDesignator::Name(name)),
            ),
            (
                "lambda function designator",
                Expr::Function(FunctionDesignator::Lambda(Box::new(lambda.clone()))),
            ),
            ("lambda", Expr::Lambda(Box::new(lambda.clone()))),
            (
                "if",
                Expr::If {
                    test: Box::new(probe()),
                    then: Box::new(probe()),
                    otherwise: Some(Box::new(probe())),
                },
            ),
            ("progn", Expr::Progn(vec![probe()])),
            (
                "block",
                Expr::Block {
                    name: SymbolRef::interned("COMMON-LISP", "NIL"),
                    body: vec![probe()],
                },
            ),
            (
                "return-from with value",
                Expr::ReturnFrom {
                    name: SymbolRef::interned("COMMON-LISP", "NIL"),
                    value: Some(Box::new(probe())),
                },
            ),
            (
                "tagbody",
                Expr::Tagbody(vec![
                    TagbodyItem::Tag(SymbolRef::interned("COMMON-LISP-USER", "TAG")),
                    TagbodyItem::Form(probe()),
                ]),
            ),
            (
                "catch",
                Expr::Catch {
                    tag: Box::new(probe()),
                    body: vec![probe()],
                },
            ),
            (
                "throw",
                Expr::Throw {
                    tag: Box::new(probe()),
                    value: Box::new(probe()),
                },
            ),
            (
                "unwind-protect",
                Expr::UnwindProtect {
                    protected: Box::new(probe()),
                    cleanup: vec![probe()],
                },
            ),
            (
                "let",
                Expr::Let {
                    sequential: false,
                    bindings: vec![
                        LetBinding {
                            name: SymbolRef::interned("COMMON-LISP-USER", "VALUE"),
                            value: Some(probe()),
                        },
                        LetBinding {
                            name: SymbolRef::interned("COMMON-LISP-USER", "NIL"),
                            value: None,
                        },
                    ],
                    declarations: vec![optimize(Quality::Debug, 3)],
                    body: vec![probe()],
                },
            ),
            (
                "progv",
                Expr::Progv {
                    symbols: Box::new(probe()),
                    values: Box::new(probe()),
                    body: vec![probe()],
                },
            ),
            (
                "setq",
                Expr::Setq(vec![(
                    SymbolRef::interned("COMMON-LISP-USER", "VALUE"),
                    probe(),
                )]),
            ),
            (
                "multiple-value-call",
                Expr::MultipleValueCall {
                    function: Box::new(probe()),
                    arguments: vec![probe()],
                },
            ),
            (
                "multiple-value-prog1",
                Expr::MultipleValueProg1 {
                    first: Box::new(probe()),
                    forms: vec![probe()],
                },
            ),
            (
                "the",
                Expr::The {
                    type_specifier: TypeSpecifier::new(Literal::Nil),
                    value: Box::new(probe()),
                },
            ),
            (
                "load-time-value",
                Expr::LoadTimeValue {
                    form: Box::new(probe()),
                    read_only: true,
                },
            ),
            (
                "locally",
                Expr::Locally {
                    declarations: vec![optimize(Quality::Debug, 3)],
                    body: vec![probe()],
                },
            ),
            (
                "flet",
                Expr::Flet {
                    definitions: vec![local_function.clone()],
                    declarations: vec![optimize(Quality::Debug, 3)],
                    body: vec![probe()],
                },
            ),
            (
                "labels",
                Expr::Labels {
                    definitions: vec![local_function],
                    declarations: vec![optimize(Quality::Debug, 3)],
                    body: vec![probe()],
                },
            ),
            (
                "macrolet",
                Expr::Macrolet {
                    definitions: vec![local_macro],
                    declarations: vec![optimize(Quality::Debug, 3)],
                    body: vec![probe()],
                },
            ),
            (
                "symbol-macrolet",
                Expr::SymbolMacrolet {
                    declarations: vec![optimize(Quality::Debug, 3)],
                    body: vec![probe()],
                    definitions: vec![symbol_macro],
                },
            ),
        ];
        for (shape, form) in forms {
            let expression = Expr::Locally {
                declarations: vec![
                    Declaration::Special(Vec::new()),
                    optimize(Quality::Debug, 3),
                ],
                body: vec![form],
            };
            assert_eq!(
                optimization_level(&expression),
                super::OptimizationLevel::Debug,
                "{shape}"
            );
        }
        assert_eq!(
            optimization_level(&Expr::Progn(Vec::new())),
            super::OptimizationLevel::Safety
        );
        assert_eq!(
            optimization_level(&Expr::Locally {
                declarations: vec![optimize(Quality::Speed, 3), optimize(Quality::Safety, 1)],
                body: Vec::new(),
            }),
            super::OptimizationLevel::Speed
        );
        assert_eq!(
            optimization_level(&Expr::Constant(Literal::Nil)),
            super::OptimizationLevel::Safety
        );
        assert_eq!(
            optimization_level(&Expr::Variable(SymbolRef::interned(
                "COMMON-LISP-USER",
                "VALUE",
            ))),
            super::OptimizationLevel::Safety
        );
    }

    #[test]
    fn unsupported_optimization_qualities_keep_the_safety_default() {
        for quality in [Quality::Space, Quality::CompilationSpeed, Quality::Unknown] {
            let expression = Expr::Locally {
                declarations: vec![optimize(quality, 3)],
                body: Vec::new(),
            };
            assert_eq!(
                optimization_level(&expression),
                super::OptimizationLevel::Safety
            );
        }
    }

    #[test]
    fn payload_round_trips_and_rejects_tampering() {
        let payload = encode_payload(b"(+ 1 2)").expect("payload encoding");
        assert_eq!(
            decode_payload(&payload).expect("payload decoding"),
            "(+ 1 2)"
        );

        let mut wrong_version = payload.clone();
        wrong_version[9] = 2;
        assert!(
            decode_payload(&wrong_version)
                .expect_err("version must be rejected")
                .to_string()
                .contains("unsupported runtime FASL payload version")
        );

        let mut wrong_hash = payload;
        let last = wrong_hash.last_mut().expect("encoded source");
        *last ^= 1;
        assert!(
            decode_payload(&wrong_hash)
                .expect_err("hash mismatch must be rejected")
                .to_string()
                .contains("runtime FASL source hash mismatch")
        );
    }

    #[test]
    fn fasl_headers_and_payload_shape_errors_are_rejected() {
        assert!(is_fasl(b"NCLFASL\0"));
        assert!(!is_fasl(b"NCLRTFASL"));
        assert!(decode_payload(&[]).is_err());

        let payload = encode_payload(b"(+ 1 2)").expect("payload encoding");
        let mut wrong_length = payload.clone();
        wrong_length[11] = 0xff;
        assert!(
            decode_payload(&wrong_length)
                .expect_err("payload length must be rejected")
                .to_string()
                .contains("invalid runtime FASL payload length")
        );

        let invalid_utf8 = encode_payload(&[0xff]).expect("binary payload encoding");
        assert!(
            decode_payload(&invalid_utf8)
                .expect_err("invalid UTF-8 must be rejected")
                .to_string()
                .contains("runtime FASL source is not valid UTF-8")
        );
    }

    #[test]
    fn read_forms_collects_forms_and_propagates_reader_errors() {
        let object = ObjectRuntime::new().expect("object runtime");
        let mut context = ThreadContext::new();
        context.register(&object).expect("register object runtime");
        ncl_stdlib::register_all(&mut context, &object).expect("register standard library");

        let forms = read_forms(&mut context, &object, "(+ 1 2) 7").expect("read forms");
        assert_eq!(forms.len(), 2);
        assert!(read_forms(&mut context, &object, "(+ 1").is_err());
    }
}
