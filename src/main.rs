//! NCL command-line entry point.

fn main() -> std::process::ExitCode {
    let mut arguments = std::env::args().skip(1);
    match arguments.next().as_deref() {
        Some("--version" | "-V") => {
            println!("ncl {}", env!("CARGO_PKG_VERSION"));
            std::process::ExitCode::SUCCESS
        }
        Some("--eval") => {
            let Some(source) = arguments.next() else {
                eprintln!("--eval requires a source string");
                return std::process::ExitCode::from(2);
            };
            let mut runtime = match ncl_runtime::Runtime::new() {
                Ok(runtime) => runtime,
                Err(error) => {
                    eprintln!("ncl: {error}");
                    return std::process::ExitCode::from(1);
                }
            };
            let result = runtime
                .eval(&source)
                .map(|value| runtime.format_result(value));
            match result {
                Ok(value) => {
                    println!("{value}");
                    std::mem::forget(runtime);
                    std::process::ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("ncl: {error}");
                    std::mem::forget(runtime);
                    std::process::ExitCode::from(1)
                }
            }
        }
        Some(argument) => {
            eprintln!("unsupported option: {argument}");
            std::process::ExitCode::from(2)
        }
        None => {
            eprintln!("ncl is being rewritten; use --version for the current version");
            std::process::ExitCode::from(2)
        }
    }
}
