//! Pure pattern-directed pathname translation.

use super::{Component, DirectoryElement, Pathname, PathnameError, pathname_match_p};

pub fn translate_pathname(
    source: &Pathname,
    from: &Pathname,
    to: &Pathname,
) -> Result<Pathname, PathnameError> {
    if !pathname_match_p(source, from) {
        return Err(PathnameError::MissingTranslationComponent);
    }
    Ok(Pathname {
        host: translate_component(source.host.as_ref(), from.host.as_ref(), to.host.as_ref()),
        device: translate_component(
            source.device.as_ref(),
            from.device.as_ref(),
            to.device.as_ref(),
        ),
        directory: translate_directory(source, from, to),
        name: translate_pattern_component(source.name.as_ref(), from.name.as_ref(), to.name.as_ref()),
        type_: translate_pattern_component(source.type_.as_ref(), from.type_.as_ref(), to.type_.as_ref()),
        version: to.version.clone().or_else(|| source.version.clone()),
    })
}

fn translate_component<T: Clone>(
    source: Option<&T>,
    _from: Option<&T>,
    to: Option<&T>,
) -> Option<T> {
    match to {
        Some(replacement) => Some(replacement.clone()),
        None => source.cloned(),
    }
}

fn translate_pattern_component(
    source: Option<&Component>,
    from: Option<&Component>,
    to: Option<&Component>,
) -> Option<Component> {
    match to {
        Some(Component::Wild | Component::WildInferiors) => {
            if matches!(from, Some(Component::Wild | Component::WildInferiors)) {
                source.cloned().or_else(|| to.cloned())
            } else {
                to.cloned()
            }
        }
        Some(replacement) => Some(replacement.clone()),
        None => source.cloned(),
    }
}

fn translate_directory(
    source: &Pathname,
    from: &Pathname,
    to: &Pathname,
) -> Option<super::Directory> {
    let Some(target) = &to.directory else {
        return source.directory.clone();
    };
    let Some(pattern) = &from.directory else {
        return Some(target.clone());
    };
    let Some(actual) = &source.directory else {
        return Some(target.clone());
    };
    let Some(captures) = match_directory_captures(&actual.elements, &pattern.elements) else {
        return Some(target.clone());
    };
    let mut capture_index = 0;
    let mut elements = Vec::new();
    for element in &target.elements {
        match element {
            DirectoryElement::Wild | DirectoryElement::WildInferiors => {
                if let Some(capture) = captures.get(capture_index) {
                    elements.extend(capture.iter().cloned());
                } else {
                    elements.push(element.clone());
                }
                capture_index += 1;
            }
            literal => elements.push(literal.clone()),
        }
    }
    Some(super::Directory {
        kind: target.kind,
        elements,
    })
}

fn match_directory_captures(
    actual: &[DirectoryElement],
    pattern: &[DirectoryElement],
) -> Option<Vec<Vec<DirectoryElement>>> {
    fn visit(
        actual: &[DirectoryElement],
        pattern: &[DirectoryElement],
        captures: &mut Vec<Vec<DirectoryElement>>,
    ) -> bool {
        let Some((pattern_element, remaining_pattern)) = pattern.split_first() else {
            return actual.is_empty();
        };
        match pattern_element {
            DirectoryElement::Wild => {
                let Some((value, remaining_actual)) = actual.split_first() else {
                    return false;
                };
                captures.push(vec![value.clone()]);
                if visit(remaining_actual, remaining_pattern, captures) {
                    return true;
                }
                captures.pop();
                false
            }
            DirectoryElement::WildInferiors => {
                for capture_length in 0..=actual.len() {
                    captures.push(actual[..capture_length].to_vec());
                    if visit(&actual[capture_length..], remaining_pattern, captures) {
                        return true;
                    }
                    captures.pop();
                }
                false
            }
            expected => {
                let Some((value, remaining_actual)) = actual.split_first() else {
                    return false;
                };
                value == expected && visit(remaining_actual, remaining_pattern, captures)
            }
        }
    }

    let mut captures = Vec::new();
    visit(actual, pattern, &mut captures).then_some(captures)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::parse_namestring;

    #[test]
    fn captures_multiple_wild_inferiors_in_pattern_order() {
        let source = parse_namestring("/a/b/f/g/file.txt").expect("source");
        let from = parse_namestring("/**/f/**/file.txt").expect("from");
        let to = parse_namestring("/out/**/middle/**/result.txt").expect("to");
        assert_eq!(
            translate_pathname(&source, &from, &to).expect("translation").namestring(),
            "/out/a/b/middle/g/result.txt"
        );
    }

    #[test]
    fn wildcard_capture_can_be_empty_without_panicking() {
        let source = parse_namestring("/file.txt").expect("source");
        let from = parse_namestring("/**/file.txt").expect("from");
        let to = parse_namestring("/out/**/result.txt").expect("to");
        assert_eq!(
            translate_pathname(&source, &from, &to).expect("translation").namestring(),
            "/out/result.txt"
        );
    }
}
