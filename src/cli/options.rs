use super::error::CliError;

#[derive(Debug, Default)]
pub(super) struct CliOptions {
    pub(super) evaluations: Vec<String>,
    pub(super) file: Option<String>,
    pub(super) repl: bool,
    pub(super) quiet: bool,
    pub(super) compiled: bool,
}

impl CliOptions {
    pub(super) fn parse(arguments: &[String]) -> Result<Self, CliError> {
        let mut options = Self::default();
        let mut index = 0;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--eval" | "-e" => {
                    index += 1;
                    let Some(source) = arguments.get(index) else {
                        return Err(CliError::Usage(
                            "--eval requires a source string".to_string(),
                        ));
                    };
                    options.evaluations.push(source.clone());
                }
                "--file" | "-f" => {
                    index += 1;
                    let Some(path) = arguments.get(index) else {
                        return Err(CliError::Usage("--file requires a path".to_string()));
                    };
                    options.file = Some(path.clone());
                }
                "--repl" => options.repl = true,
                "--compiled" => options.compiled = true,
                "--quiet" | "-q" => options.quiet = true,
                argument if argument.starts_with('-') => {
                    return Err(CliError::Usage(format!("unknown option {argument}")));
                }
                path => {
                    return Err(CliError::Usage(format!(
                        "unexpected argument {path}; use --file"
                    )));
                }
            }
            index += 1;
        }
        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    use super::CliOptions;

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn parses_all_execution_options() -> Result<(), String> {
        let options = CliOptions::parse(&arguments(&[
            "-e",
            "(+ 1 2)",
            "--eval",
            "42",
            "-f",
            "input.lisp",
            "--repl",
            "--compiled",
            "-q",
        ]))
        .map_err(|error| format!("valid options should parse: {error:?}"))?;

        assert_eq!(options.evaluations, ["(+ 1 2)", "42"]);
        assert_eq!(options.file.as_deref(), Some("input.lisp"));
        assert!(options.repl);
        assert!(options.compiled);
        assert!(options.quiet);
        Ok(())
    }

    #[test]
    fn rejects_missing_values_and_unexpected_arguments() -> Result<(), String> {
        for (values, expected) in [
            (&["--eval"][..], "--eval requires a source string"),
            (&["--file"][..], "--file requires a path"),
            (&["input.lisp"][..], "unexpected argument input.lisp"),
        ] {
            let error = CliOptions::parse(&arguments(values))
                .err()
                .ok_or_else(|| "options should fail".to_string())?;
            assert!(format!("{error:?}").contains(expected));
        }
        Ok(())
    }

    #[test]
    fn rejects_unknown_options() -> Result<(), String> {
        let error = CliOptions::parse(&arguments(&["--unknown"]))
            .err()
            .ok_or_else(|| "unknown options should fail".to_string())?;
        assert!(format!("{error:?}").contains("unknown option --unknown"));
        Ok(())
    }
}
