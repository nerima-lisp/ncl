# ANSI-Test SBCL baseline

- Measured: 2026-10-17
- Platform: macOS arm64
- SBCL: 2.6.0
- ansi-test commit: `ca06bd919661af162c67407c9d994e881870bdb3`
- Registered tests: 21944
- Removed by the runner: 2 (LOAD-PATHNAME.1, LOAD-TRUENAME.1)
- SBCL process exit status: 0
- Executed: 21938
- Disabled by an ANSI-Test note: 4
- Passed: 21764
- Failed: 174
- Unexecuted active tests: 0

The runner removes `LOAD-PATHNAME.1` and `LOAD-TRUENAME.1` because upstream `doit.lsp` says they misbehave when invoked from a file being loaded.

The note-disabled tests are `OPEN.IO.21`, `OPEN.IO.30`, `OPEN.OUTPUT.21`, `OPEN.OUTPUT.30`.

The note assumes `OPEN` with `:if-exists :new-version` does not fail; ansi-test disables it for SBCL.

Passing test names are in the adjacent `.txt` file; the chapter map is in `.tsv`.
