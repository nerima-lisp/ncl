#![expect(
    clippy::redundant_pub_crate,
    reason = "items stay crate-visible; workspace will enable unreachable_pub, which conflicts with this nursery lint"
)]
#![expect(
    dead_code,
    reason = "Each integration test compiles this module independently and uses only the helpers it needs."
)]

use std::io::{self, Read, Write};
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, ExitStatus, Output, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const DEFAULT_TIMEOUTS: ChildTimeouts = ChildTimeouts::new(30, Duration::from_secs(300));
const MAX_EVAL_BATCH_SIZE: usize = 5;

#[derive(Clone, Debug)]
pub(super) struct CliEvalCase {
    pub(super) name: String,
    pub(super) source: String,
    pub(super) expected: String,
}

impl CliEvalCase {
    pub(super) fn new(
        name: impl Into<String>,
        source: impl Into<String>,
        expected: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            source: source.into(),
            expected: expected.into(),
        }
    }
}

pub(super) fn assert_ncl_eval_cases(cases: &[CliEvalCase]) {
    assert!(!cases.is_empty(), "an E2E batch must contain cases");

    for (batch_index, batch) in cases.chunks(MAX_EVAL_BATCH_SIZE).enumerate() {
        let first_case_index = batch_index * MAX_EVAL_BATCH_SIZE;
        let forms = batch
            .iter()
            .enumerate()
            .map(|(index, case)| {
                let case_index = first_case_index + index;
                let label = lisp_string_literal(&format!("{}#{case_index}", case.name));
                let expected = lisp_string_literal(&case.expected);
                format!(
                    "(handler-case \
                        (let* ((*random-state* (make-random-state nil)) \
                               (ncl-e2e-batch-value-{case_index} {}) \
                               (ncl-e2e-batch-printed-{case_index} \
                                   (format nil \"~S\" ncl-e2e-batch-value-{case_index}))) \
                          (unless (string= ncl-e2e-batch-printed-{case_index} {expected}) \
                            (list {label} :mismatch \
                                  ncl-e2e-batch-printed-{case_index} {expected}))) \
                      (condition (ncl-e2e-batch-condition-{case_index}) \
                        (list {label} :condition)))",
                    case.source
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        let source = format!("(remove nil (list {forms}))");
        let case_names = batch
            .iter()
            .map(|case| case.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let output = run_ncl(&source);

        assert_eq!(
            output.status.code(),
            Some(0),
            "batch {batch_index} ({case_names}) exited with stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.stderr.is_empty(),
            "batch {batch_index} ({case_names}) wrote to stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "NIL\n",
            "batch {batch_index} ({case_names}) returned a mismatched case or wrote to stdout"
        );
    }
}

fn lisp_string_literal(value: &str) -> String {
    let mut literal = String::with_capacity(value.len() + 2);
    literal.push('"');
    for character in value.chars() {
        if matches!(character, '"' | '\\') {
            literal.push('\\');
        }
        literal.push(character);
    }
    literal.push('"');
    literal
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ChildTimeouts {
    cpu_seconds: u64,
    wall_clock: Duration,
}

impl ChildTimeouts {
    pub(super) const fn new(cpu_seconds: u64, wall_clock: Duration) -> Self {
        Self {
            cpu_seconds,
            wall_clock,
        }
    }
}

#[derive(Debug)]
pub(super) enum ChildRunError {
    CpuTimeLimitExceeded {
        signal: i32,
    },
    WallClockLimitExceeded {
        signal: Option<i32>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    CpuLimitSetup(io::Error),
    Spawn(io::Error),
    Poll(io::Error),
    Wait(io::Error),
    KillFailed {
        error: io::Error,
        status: Option<ExitStatus>,
    },
    KillAndPollFailed {
        kill_error: io::Error,
        poll_error: io::Error,
    },
    MissingStdin,
    StdinWrite(io::Error),
    MissingOutputPipe,
    StreamRead(io::Error),
    StreamReaderPanicked,
}

pub(super) fn run_ncl(source: &str) -> Output {
    run_ncl_with_limits(source, DEFAULT_TIMEOUTS)
        .unwrap_or_else(|error| panic!("{source}: ncl child failed: {error:?}"))
}

pub(super) fn run_ncl_with_limits(
    source: &str,
    timeouts: ChildTimeouts,
) -> Result<Output, ChildRunError> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ncl"));
    command.args(["--eval", source]).stdin(Stdio::null());
    run_command_with_limits(&mut command, None, timeouts)
}

pub(super) fn run_command(command: &mut Command) -> Output {
    run_command_with_input(command, None)
        .unwrap_or_else(|error| panic!("ncl child failed: {error:?}"))
}

pub(super) fn run_command_with_stdin(command: &mut Command, input: &[u8]) -> Output {
    run_command_with_input(command, Some(input))
        .unwrap_or_else(|error| panic!("ncl child failed: {error:?}"))
}

fn run_command_with_input(
    command: &mut Command,
    input: Option<&[u8]>,
) -> Result<Output, ChildRunError> {
    run_command_with_limits(command, input, DEFAULT_TIMEOUTS)
}

pub(super) fn run_command_with_limits(
    command: &mut Command,
    input: Option<&[u8]>,
    timeouts: ChildTimeouts,
) -> Result<Output, ChildRunError> {
    ncl_sys::set_child_cpu_time_limit(command, timeouts.cpu_seconds)
        .map_err(ChildRunError::CpuLimitSetup)?;
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if input.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command.spawn().map_err(ChildRunError::Spawn)?;
    let deadline = Instant::now() + timeouts.wall_clock;

    let Some(stdout) = child.stdout.take() else {
        terminate(&mut child)?;
        return Err(ChildRunError::MissingOutputPipe);
    };
    let Some(stderr) = child.stderr.take() else {
        terminate(&mut child)?;
        return Err(ChildRunError::MissingOutputPipe);
    };
    let stdout_reader = read_stream(stdout);
    let stderr_reader = read_stream(stderr);

    if let Some(input) = input {
        let Some(mut stdin) = child.stdin.take() else {
            terminate(&mut child)?;
            collect_streams(stdout_reader, stderr_reader)?;
            return Err(ChildRunError::MissingStdin);
        };
        if let Err(error) = stdin.write_all(input) {
            terminate(&mut child)?;
            collect_streams(stdout_reader, stderr_reader)?;
            return Err(ChildRunError::StdinWrite(error));
        }
    } else {
        drop(child.stdin.take());
    }

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let (stdout, stderr) = collect_streams(stdout_reader, stderr_reader)?;
                if let Some(signal) = cpu_limit_signal(status) {
                    return Err(ChildRunError::CpuTimeLimitExceeded { signal });
                }
                return Ok(Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => match child.kill() {
                Ok(()) => {
                    let status = child.wait().map_err(ChildRunError::Wait)?;
                    let (stdout, stderr) = collect_streams(stdout_reader, stderr_reader)?;
                    return Err(ChildRunError::WallClockLimitExceeded {
                        signal: status.signal(),
                        stdout,
                        stderr,
                    });
                }
                Err(error) => match child.try_wait() {
                    Ok(Some(status)) => {
                        collect_streams(stdout_reader, stderr_reader)?;
                        if let Some(signal) = cpu_limit_signal(status) {
                            return Err(ChildRunError::CpuTimeLimitExceeded { signal });
                        }
                        return Err(ChildRunError::KillFailed {
                            error,
                            status: Some(status),
                        });
                    }
                    Ok(None) => {
                        return Err(ChildRunError::KillFailed {
                            error,
                            status: None,
                        });
                    }
                    Err(poll_error) => {
                        return Err(ChildRunError::KillAndPollFailed {
                            kill_error: error,
                            poll_error,
                        });
                    }
                },
            },
            Err(error) => {
                terminate(&mut child)?;
                collect_streams(stdout_reader, stderr_reader)?;
                return Err(ChildRunError::Poll(error));
            }
        }
    }
}

fn read_stream<R: Read + Send + 'static>(mut stream: R) -> JoinHandle<io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut output = Vec::new();
        stream.read_to_end(&mut output).map(|_| output)
    })
}

fn collect_streams(
    stdout_reader: JoinHandle<io::Result<Vec<u8>>>,
    stderr_reader: JoinHandle<io::Result<Vec<u8>>>,
) -> Result<(Vec<u8>, Vec<u8>), ChildRunError> {
    let stdout = collect_stream(stdout_reader);
    let stderr = collect_stream(stderr_reader);
    match (stdout, stderr) {
        (Ok(stdout), Ok(stderr)) => Ok((stdout, stderr)),
        (Err(error), _) | (_, Err(error)) => Err(error),
    }
}

fn collect_stream(reader: JoinHandle<io::Result<Vec<u8>>>) -> Result<Vec<u8>, ChildRunError> {
    reader
        .join()
        .map_err(|_| ChildRunError::StreamReaderPanicked)?
        .map_err(ChildRunError::StreamRead)
}

fn terminate(child: &mut std::process::Child) -> Result<(), ChildRunError> {
    match child.kill() {
        Ok(()) => child.wait().map(|_| ()).map_err(ChildRunError::Wait),
        Err(error) => match child.try_wait() {
            Ok(Some(_)) => Ok(()),
            Ok(None) => Err(ChildRunError::KillFailed {
                error,
                status: None,
            }),
            Err(poll_error) => Err(ChildRunError::KillAndPollFailed {
                kill_error: error,
                poll_error,
            }),
        },
    }
}

fn cpu_limit_signal(status: ExitStatus) -> Option<i32> {
    status
        .signal()
        .filter(|signal| *signal == 9 || *signal == 24)
}

pub(super) fn assert_eval_with_stress(source: &str, expected: &str) {
    let mut runtime = ncl_runtime::Runtime::new()
        .unwrap_or_else(|error| panic!("runtime initialization failed: {error:?}"));
    assert_eval_with_stress_on(&mut runtime, source, expected);
}

pub(super) fn assert_eval_with_stress_on(
    runtime: &mut ncl_runtime::Runtime,
    source: &str,
    expected: &str,
) {
    runtime.set_gc_stress(true);
    runtime.set_strict_forwarding(true);
    let value = runtime
        .compile(source)
        .unwrap_or_else(|error| panic!("compile failed for {source}: {error:?}"));
    assert_eq!(runtime.format_result(value), expected, "{source}");
}
