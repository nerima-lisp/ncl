use ncl_runtime::Runtime;

pub fn evaluate_interpreted(source: &str) -> Result<String, String> {
    Runtime::new()
        .eval_source(source)
        .map_err(|error| error.to_string())
        .and_then(|mut values| {
            values
                .pop()
                .map(|value| value.to_string())
                .ok_or_else(|| "evaluation returned no values".to_string())
        })
}

pub fn evaluate_compiled(source: &str) -> Result<String, String> {
    Runtime::new()
        .eval_compiled_source(source)
        .map_err(|error| error.to_string())
        .and_then(|mut values| {
            values
                .pop()
                .map(|value| value.to_string())
                .ok_or_else(|| "compiled evaluation returned no values".to_string())
        })
}

pub fn assert_interpreted_and_compiled(source: &str, expected: &str) {
    let interpreted = evaluate_interpreted(source);
    let compiled = evaluate_compiled(source);

    assert_eq!(
        interpreted,
        Ok(expected.to_string()),
        "interpreted evaluation of {source:?}",
    );
    assert_eq!(
        compiled,
        Ok(expected.to_string()),
        "compiled evaluation of {source:?}",
    );
}
