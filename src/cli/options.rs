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
        let mut options_ended = false;
        while index < arguments.len() {
            let argument = arguments[index].as_str();
            match argument {
                "--" if !options_ended => options_ended = true,
                "--eval" | "-e" if !options_ended => {
                    index += 1;
                    let Some(source) = arguments.get(index) else {
                        return Err(CliError::Usage(
                            "--eval requires a source string".to_string(),
                        ));
                    };
                    options.evaluations.push(source.clone());
                }
                "--file" | "-f" if !options_ended => {
                    index += 1;
                    let Some(path) = arguments.get(index) else {
                        return Err(CliError::Usage("--file requires a path".to_string()));
                    };
                    if options.file.is_some() {
                        return Err(CliError::Usage("--file may only be given once".to_string()));
                    }
                    options.file = Some(path.clone());
                }
                "--repl" if !options_ended => options.repl = true,
                "--compiled" if !options_ended => options.compiled = true,
                "--quiet" | "-q" if !options_ended => options.quiet = true,
                _ if !options_ended && argument.starts_with('-') => {
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

    #[test]
    fn rejects_a_repeated_file_option() -> Result<(), String> {
        let error = CliOptions::parse(&arguments(&["--file", "a", "--file", "b"]))
            .err()
            .ok_or_else(|| "repeated --file should fail".to_string())?;
        assert!(format!("{error:?}").contains("--file may only be given once"));
        Ok(())
    }

    #[test]
    fn double_dash_ends_option_parsing() -> Result<(), String> {
        let error = CliOptions::parse(&arguments(&["--", "--repl"]))
            .err()
            .ok_or_else(|| "an argument after -- should not be parsed as an option".to_string())?;
        assert!(
            format!("{error:?}").contains("unexpected argument --repl"),
            "a flag-shaped argument after -- must be treated as a positional argument, not \
             re-enter option parsing"
        );
        Ok(())
    }

    #[test]
    fn a_second_double_dash_after_the_first_is_a_positional_argument() -> Result<(), String> {
        let error = CliOptions::parse(&arguments(&["--", "--"]))
            .err()
            .ok_or_else(|| "a second -- should be a positional argument".to_string())?;
        assert!(format!("{error:?}").contains("unexpected argument --"));
        Ok(())
    }
}
