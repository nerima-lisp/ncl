use std::collections::HashSet;

use ncl_hir::form::{HirDeclaration, HirQuality, OptimizationSetting, ScopeMetadata};
use ncl_hir::{OptimizationPolicy, OptimizationQuality, ScopeId, SymbolId};

use super::HirBuildError;
use super::context::BuildContext;
use crate::declaration::{Declaration, Quality};

impl BuildContext<'_> {
    pub(super) fn attach_declarations(
        &mut self,
        scope: ScopeId,
        declarations: &[Declaration],
    ) -> Result<(), HirBuildError> {
        let mut hir_declarations = Vec::with_capacity(declarations.len());
        let mut policy = self
            .policies
            .last()
            .copied()
            .unwrap_or_else(OptimizationPolicy::default);

        for declaration in declarations {
            let hir_declaration = match declaration {
                Declaration::Special(names) => {
                    let names = names
                        .iter()
                        .map(|name| self.intern(name))
                        .collect::<Result<Vec<_>, _>>()?;
                    for name in &names {
                        self.add_special(scope, *name);
                    }
                    HirDeclaration::Special { names }
                }
                Declaration::Type {
                    type_specifier,
                    names,
                } => {
                    let (type_id, source_specifier) = self.type_specifier(type_specifier)?;
                    let names = names
                        .iter()
                        .map(|name| self.intern(name))
                        .collect::<Result<Vec<_>, _>>()?;
                    HirDeclaration::Type {
                        type_id,
                        source_specifier,
                        names,
                    }
                }
                Declaration::Ftype {
                    type_specifier,
                    name,
                } => {
                    let (type_id, source_specifier) = self.type_specifier(type_specifier)?;
                    HirDeclaration::Ftype {
                        type_id,
                        source_specifier,
                        name: self.intern(name)?,
                    }
                }
                Declaration::Inline(names) => HirDeclaration::Inline {
                    names: names
                        .iter()
                        .map(|name| self.intern(name))
                        .collect::<Result<Vec<_>, _>>()?,
                },
                Declaration::Notinline(names) => HirDeclaration::Notinline {
                    names: names
                        .iter()
                        .map(|name| self.intern(name))
                        .collect::<Result<Vec<_>, _>>()?,
                },
                Declaration::Optimize(settings) => {
                    let mut hir_settings = Vec::with_capacity(settings.len());
                    for setting in settings {
                        let quality = match setting.quality {
                            Quality::Speed => {
                                policy =
                                    policy.with_speed(OptimizationQuality::new(setting.value)?);
                                HirQuality::Speed
                            }
                            Quality::Space => {
                                policy =
                                    policy.with_space(OptimizationQuality::new(setting.value)?);
                                HirQuality::Space
                            }
                            Quality::Safety => {
                                policy =
                                    policy.with_safety(OptimizationQuality::new(setting.value)?);
                                HirQuality::Safety
                            }
                            Quality::Debug => {
                                policy =
                                    policy.with_debug(OptimizationQuality::new(setting.value)?);
                                HirQuality::Debug
                            }
                            Quality::CompilationSpeed => HirQuality::CompilationSpeed,
                            Quality::Unknown => {
                                return Err(HirBuildError::Unsupported(
                                    super::UnsupportedHirForm::UnknownOptimizationQuality,
                                ));
                            }
                        };
                        hir_settings.push(OptimizationSetting {
                            quality,
                            value: setting.value,
                        });
                    }
                    HirDeclaration::Optimize {
                        settings: hir_settings,
                    }
                }
                Declaration::DynamicExtent(names) => HirDeclaration::DynamicExtent {
                    names: names
                        .iter()
                        .map(|name| self.intern(name))
                        .collect::<Result<Vec<_>, _>>()?,
                },
                Declaration::Ignore(names) => HirDeclaration::Ignore {
                    names: names
                        .iter()
                        .map(|name| self.intern(name))
                        .collect::<Result<Vec<_>, _>>()?,
                },
                Declaration::Ignorable(names) => HirDeclaration::Ignorable {
                    names: names
                        .iter()
                        .map(|name| self.intern(name))
                        .collect::<Result<Vec<_>, _>>()?,
                },
                Declaration::Unknown { name, arguments } => HirDeclaration::Unknown {
                    name: self.intern(name)?,
                    arguments: arguments
                        .iter()
                        .map(|argument| self.literal(argument))
                        .collect::<Result<Vec<_>, _>>()?,
                },
            };
            hir_declarations.push(hir_declaration);
        }

        self.form.set_scope_metadata(
            scope,
            ScopeMetadata {
                declarations: hir_declarations,
                policy,
            },
        )?;
        if scope == self.form.root_scope() {
            self.form.set_policy(policy);
        }
        if let Some(current) = self.policies.last_mut() {
            *current = policy;
        }
        Ok(())
    }

    pub(super) fn special_declarations(
        &mut self,
        declarations: &[Declaration],
    ) -> Result<HashSet<SymbolId>, HirBuildError> {
        declarations
            .iter()
            .filter_map(|declaration| match declaration {
                Declaration::Special(names) => Some(names.as_slice()),
                _ => None,
            })
            .flatten()
            .map(|name| self.intern(name))
            .collect()
    }

    pub(super) fn lower_declared_sequence(
        &mut self,
        scope: ScopeId,
        declarations: &[Declaration],
        body: &[crate::ast::Expr],
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        let body_scope = self.form.bindings_mut().push_scope(scope)?;
        self.push_scope_maps();
        self.attach_declarations(body_scope, declarations)?;
        let forms = body
            .iter()
            .map(|form| self.lower_expr(form, body_scope))
            .collect::<Result<Vec<_>, _>>()?;
        self.pop_scope_maps();
        self.node(scope, ncl_hir::NodeKind::Sequence { body_scope, forms })
    }
}
