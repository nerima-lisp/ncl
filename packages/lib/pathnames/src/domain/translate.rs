//! Pure pattern-directed pathname translation.

use super::{pathname_match_p, Component, DirectoryElement, Pathname, PathnameError};

pub fn translate_pathname(source: &Pathname, from: &Pathname, to: &Pathname) -> Result<Pathname, PathnameError> {
    if !pathname_match_p(source, from) { return Err(PathnameError::MissingTranslationComponent); }
    Ok(Pathname {
        host: translate_component(source.host.as_ref(), from.host.as_ref(), to.host.as_ref()),
        device: translate_component(source.device.as_ref(), from.device.as_ref(), to.device.as_ref()),
        directory: translate_directory(source, from, to),
        name: translate_pattern_component(source.name.as_ref(), to.name.as_ref()),
        type_: translate_pattern_component(source.type_.as_ref(), to.type_.as_ref()),
        version: to.version.clone().or_else(|| source.version.clone()),
    })
}

fn translate_component<T: Clone>(source: Option<&T>, _from: Option<&T>, to: Option<&T>) -> Option<T> {
    match to {
        Some(replacement) => Some(replacement.clone()),
        None => source.cloned(),
    }
}

fn translate_pattern_component(source: Option<&Component>, to: Option<&Component>) -> Option<Component> {
    match to {
        Some(Component::Wild | Component::WildInferiors) => source.cloned(),
        Some(replacement) => Some(replacement.clone()),
        None => source.cloned(),
    }
}

fn translate_directory(source: &Pathname, from: &Pathname, to: &Pathname) -> Option<super::Directory> {
    let Some(target) = &to.directory else { return source.directory.clone() };
    let Some(pattern) = &from.directory else { return Some(target.clone()) };
    let Some(actual) = &source.directory else { return Some(target.clone()) };
    let elements = target.elements.iter().map(|element| match element {
        DirectoryElement::Wild => actual.elements.first().cloned().unwrap_or(DirectoryElement::Wild),
        DirectoryElement::WildInferiors => actual.elements.clone().into_iter().next().unwrap_or(DirectoryElement::WildInferiors),
        literal => literal.clone(),
    }).collect();
    let _ = pattern;
    Some(super::Directory { kind: target.kind, elements })
}

#[allow(dead_code)]
fn _component_is_wild(component: &Component) -> bool { !matches!(component, Component::Literal(_)) }
