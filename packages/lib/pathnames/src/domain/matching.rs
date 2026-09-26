//! Wildcard classification and pure pathname matching.

use super::{Component, DirectoryElement, Pathname};

pub fn wild_pathname_p(pathname: &Pathname) -> bool {
    pathname.name.as_ref().is_some_and(is_wild_component)
        || pathname.type_.as_ref().is_some_and(is_wild_component)
        || pathname.directory.as_ref().is_some_and(|directory| directory.elements.iter().any(is_wild_directory))
}

fn is_wild_component(component: &Component) -> bool { !matches!(component, Component::Literal(_)) }
fn is_wild_directory(element: &DirectoryElement) -> bool { matches!(element, DirectoryElement::Wild | DirectoryElement::WildInferiors) }

pub fn pathname_match_p(pathname: &Pathname, pattern: &Pathname) -> bool {
    pathname.host == pattern.host
        && pathname.device == pattern.device
        && match_directory(pathname, pattern)
        && match_component(pathname.name.as_ref(), pattern.name.as_ref())
        && match_component(pathname.type_.as_ref(), pattern.type_.as_ref())
        && pathname.version == pattern.version
}

fn match_component(value: Option<&Component>, pattern: Option<&Component>) -> bool {
    match pattern {
        None => value.is_none(),
        Some(Component::Wild | Component::WildInferiors) => value.is_some(),
        Some(Component::Literal(expected)) => matches!(value, Some(Component::Literal(actual)) if actual == expected),
    }
}

fn match_directory(value: &Pathname, pattern: &Pathname) -> bool {
    let (Some(actual), Some(expected)) = (value.directory.as_ref(), pattern.directory.as_ref()) else { return value.directory == pattern.directory };
    actual.kind == expected.kind && match_elements(&actual.elements, &expected.elements)
}

fn match_elements(actual: &[DirectoryElement], pattern: &[DirectoryElement]) -> bool {
    let Some((expected, remaining_pattern)) = pattern.split_first() else { return actual.is_empty() };
    match expected {
        DirectoryElement::WildInferiors => match_elements(actual, remaining_pattern)
            || actual.split_first().is_some_and(|(_, remaining_actual)| match_elements(remaining_actual, pattern)),
        DirectoryElement::Wild => actual.split_first().is_some_and(|(_, remaining_actual)| match_elements(remaining_actual, remaining_pattern)),
        expected => actual.split_first().is_some_and(|(actual, remaining_actual)| actual == expected && match_elements(remaining_actual, remaining_pattern)),
    }
}
