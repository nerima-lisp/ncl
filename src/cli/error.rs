use ncl_runtime::RuntimeError;

#[derive(Debug)]
pub(super) enum CliError {
    Usage(String),
    Runtime(RuntimeError),
    Io(String),
}
