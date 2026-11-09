//! Instrumented NCL runner for pass-level allocation diagnostics.

use ncl_sys::AllocationTrackingScope;
use std::io::{BufRead, Write};

#[derive(Debug, Eq, PartialEq)]
enum ProfilerOptionError {
    UnsupportedOptimizationMode(String),
}

impl std::fmt::Display for ProfilerOptionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedOptimizationMode(mode) => {
                write!(formatter, "unsupported optimization mode {mode:?}")
            }
        }
    }
}

impl std::error::Error for ProfilerOptionError {}

#[global_allocator]
static ALLOCATOR: ncl_sys::AllocationTrackingAllocator = ncl_sys::AllocationTrackingAllocator;

fn main() -> std::process::ExitCode {
    ALLOCATOR.activate();
    let (runtime_options, allocation_diagnostics, arguments) =
        match parse_runtime_options(std::env::args().skip(1)) {
            Ok(parsed) => parsed,
            Err(error) => {
                eprintln!("ncl-profiler: {error}");
                return std::process::ExitCode::from(2);
            }
        };
    let mut arguments = arguments.into_iter();
    match arguments.next().as_deref() {
        Some("--help" | "-h") => {
            print_help();
            std::process::ExitCode::SUCCESS
        }
        Some("--version" | "-V") => {
            println!("ncl-profiler {} (instrumented)", env!("CARGO_PKG_VERSION"));
            std::process::ExitCode::SUCCESS
        }
        Some("--eval") => {
            let Some(source) = arguments.next() else {
                eprintln!("--eval requires a source string");
                return std::process::ExitCode::from(2);
            };
            if arguments.next().is_some() {
                eprintln!("--eval accepts exactly one source string");
                return std::process::ExitCode::from(2);
            }
            let mut runtime = match runtime_with_options(runtime_options, allocation_diagnostics) {
                Ok(runtime) => runtime,
                Err(error) => {
                    eprintln!("ncl-profiler: {error}");
                    return std::process::ExitCode::from(1);
                }
            };
            match runtime
                .compile(&source)
                .map(|value| runtime.format_result(value))
            {
                Ok(value) => {
                    println!("{value}");
                    std::process::ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("ncl-profiler: {error}");
                    std::process::ExitCode::from(1)
                }
            }
        }
        Some(mode @ ("--load" | "--script" | "--compile-file")) => {
            let Some(path) = arguments.next() else {
                eprintln!("{mode} requires a file path");
                return std::process::ExitCode::from(2);
            };
            if arguments.next().is_some() {
                eprintln!("{mode} accepts exactly one file path");
                return std::process::ExitCode::from(2);
            }
            let mut runtime = match runtime_with_options(runtime_options, allocation_diagnostics) {
                Ok(runtime) => runtime,
                Err(error) => {
                    eprintln!("ncl-profiler: {error}");
                    return std::process::ExitCode::from(1);
                }
            };
            let result = if mode == "--compile-file" {
                runtime.compile_file(&path)
            } else {
                runtime.load_file(&path)
            };
            match result {
                Ok(value) if mode != "--script" => {
                    println!("{}", runtime.format_result(value));
                    std::process::ExitCode::SUCCESS
                }
                Ok(_) => std::process::ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("ncl-profiler: {error}");
                    std::process::ExitCode::from(1)
                }
            }
        }
        Some(argument) => {
            eprintln!("unsupported option: {argument}");
            std::process::ExitCode::from(2)
        }
        None => repl(runtime_options, allocation_diagnostics),
    }
}

fn parse_runtime_options(
    arguments: impl Iterator<Item = String>,
) -> Result<(ncl_runtime::RuntimeOptions, bool, Vec<String>), ProfilerOptionError> {
    let mut options = ncl_runtime::RuntimeOptions::default();
    let mut allocation_diagnostics = false;
    let mut remaining = Vec::new();
    let mut parsing_options = true;
    for argument in arguments {
        if parsing_options {
            if argument == "--opt-diagnostics" {
                options.opt_diagnostics = true;
                allocation_diagnostics = true;
                continue;
            }
            if let Some(mode) = argument.strip_prefix("--optimization=") {
                options.optimization = if mode == "default" {
                    ncl_runtime::OptimizationMode::Default
                } else if mode == "off" {
                    ncl_runtime::OptimizationMode::Off
                } else {
                    return Err(ProfilerOptionError::UnsupportedOptimizationMode(
                        mode.to_owned(),
                    ));
                };
                continue;
            }
            parsing_options = false;
        }
        remaining.push(argument);
    }
    Ok((options, allocation_diagnostics, remaining))
}

fn runtime_with_options(
    options: ncl_runtime::RuntimeOptions,
    allocation_diagnostics: bool,
) -> Result<ncl_runtime::Runtime, ncl_runtime::RuntimeError> {
    if allocation_diagnostics && options.optimization == ncl_runtime::OptimizationMode::Default {
        let scope = AllocationTrackingScope::start()
            .map_err(|error| ncl_runtime::RuntimeError::Native(error.to_string()))?;
        ncl_runtime::Runtime::with_options_and_pass_runner(options, move |passes, module| {
            passes.run_with_observer_and_allocation_snapshot(
                module,
                |event| eprintln!("{event}"),
                || scope.allocated_bytes(),
            )
        })
    } else {
        ncl_runtime::Runtime::with_options(options)
    }
}

fn repl(
    options: ncl_runtime::RuntimeOptions,
    allocation_diagnostics: bool,
) -> std::process::ExitCode {
    let mut runtime = match runtime_with_options(options, allocation_diagnostics) {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("ncl-profiler: {error}");
            return std::process::ExitCode::from(1);
        }
    };
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut line = String::new();
    let mut form = String::new();
    loop {
        eprint!("> ");
        if let Err(error) = std::io::stderr().flush() {
            eprintln!("ncl-profiler: {error}");
            return std::process::ExitCode::from(1);
        }
        line.clear();
        match input.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                form.push_str(&line);
                match runtime.eval(&form) {
                    Ok(value) => println!("{}", runtime.format_result(value)),
                    Err(error) if error.is_incomplete_read() => continue,
                    Err(error) => eprintln!("ncl-profiler: {error}"),
                }
                form.clear();
            }
            Err(error) => {
                eprintln!("ncl-profiler: {error}");
                return std::process::ExitCode::from(1);
            }
        }
    }
    std::process::ExitCode::SUCCESS
}

fn print_help() {
    println!(
        "Usage: ncl-profiler --opt-diagnostics --script <file>\n\
         Select --optimization=default or --optimization=off when needed.\n\
         --opt-diagnostics adds per-pass allocated_bytes to elapsed_ns diagnostics.\n\
         allocated_bytes counts requested Rust allocator bytes in pass bodies on the\n\
         pass-running thread (successful alloc, alloc_zeroed, and realloc new sizes;\n\
         frees are not subtracted). This instrumentation changes timing. Use `ncl`\n\
         for ordinary compile-performance measurements."
    );
}

#[cfg(test)]
mod tests {
    use super::parse_runtime_options;
    use ncl_sys::AllocationTrackingScope;

    #[test]
    fn allocation_diagnostics_are_only_enabled_by_the_profiler_option() {
        let (options, allocation_diagnostics, arguments) =
            parse_runtime_options(std::iter::empty())
                .unwrap_or_else(|error| panic!("options: {error}"));
        assert!(arguments.is_empty());
        assert!(!options.opt_diagnostics);
        assert!(!allocation_diagnostics);

        let (options, allocation_diagnostics, arguments) = parse_runtime_options(
            ["--opt-diagnostics", "--script", "driver.lisp"]
                .into_iter()
                .map(str::to_owned),
        )
        .unwrap_or_else(|error| panic!("options: {error}"));
        assert!(options.opt_diagnostics);
        assert!(allocation_diagnostics);
        assert_eq!(arguments, ["--script", "driver.lisp"]);
    }

    #[test]
    #[expect(
        clippy::expect_used,
        reason = "the test installs the allocator before starting its scope"
    )]
    fn installed_global_allocator_counts_requests_inside_the_scope() {
        super::ALLOCATOR.activate();
        let scope = AllocationTrackingScope::start().expect("profiler tracker is installed");
        let values = Vec::<u8>::with_capacity(123);
        assert_eq!(scope.allocated_bytes(), Some(123));
        drop(values);
        assert_eq!(scope.allocated_bytes(), Some(123));
    }
}
