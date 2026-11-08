//! Owned data and construction helpers for one top-level HIR form.

mod build_validation;
mod builder;
mod error;
mod metadata;
mod storage;
mod verify;

pub use builder::HirFormBuilder;
pub use error::FormError;
pub use metadata::{
    FormMetadata, HirDeclaration, HirQuality, LambdaAux, LambdaKey, LambdaOptional,
    LambdaSignature, LambdaSignatureKey, OptimizationSetting, ScopeMetadata,
};
pub use storage::{HirForm, TypeSpecConstantArena};
