//! Minimal command-line entry point during the native rewrite.

fn main() -> std::process::ExitCode {
    let mut arguments = std::env::args().skip(1);
    match arguments.next().as_deref() {
        Some("--version" | "-V") => {
            println!("ncl {}", env!("CARGO_PKG_VERSION"));
            std::process::ExitCode::SUCCESS
        }
        Some("--eval") => {
            eprintln!("--eval is not implemented during the native rewrite");
            std::process::ExitCode::from(1)
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
