pub fn translate_wildcards(source: &str, target: &str, value: &str) -> Option<String> {
    wildcard_captures(source, value).map(|captures| substitute_wildcards(target, &captures))
}

fn wildcard_captures(pattern: &str, value: &str) -> Option<Vec<String>> {
    fn search(
        pattern: &[char],
        value: &[char],
        pattern_index: usize,
        value_index: usize,
        captures: &mut Vec<String>,
    ) -> bool {
        if pattern_index == pattern.len() {
            return value_index == value.len();
        }
        let Some(&token) = pattern.get(pattern_index) else {
            return false;
        };
        if token == '*' {
            for end in value_index..=value.len() {
                captures.push(
                    value
                        .iter()
                        .skip(value_index)
                        .take(end - value_index)
                        .collect(),
                );
                if search(pattern, value, pattern_index + 1, end, captures) {
                    return true;
                }
                let _ = captures.pop();
            }
            return false;
        }
        let Some(&value_token) = value.get(value_index) else {
            return false;
        };
        if token != '?' && token != value_token {
            return false;
        }
        search(pattern, value, pattern_index + 1, value_index + 1, captures)
    }

    let pattern = pattern.chars().collect::<Vec<_>>();
    let value = value.chars().collect::<Vec<_>>();
    let mut captures = Vec::new();
    search(&pattern, &value, 0, 0, &mut captures).then_some(captures)
}

fn substitute_wildcards(pattern: &str, captures: &[String]) -> String {
    let mut result = String::new();
    let mut capture = captures.iter();
    for token in pattern.chars() {
        if token == '*' {
            if let Some(value) = capture.next() {
                result.push_str(value);
            }
        } else {
            result.push(token);
        }
    }
    result
}
