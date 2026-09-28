#!/bin/sh
set -u

ncl=${1:?usage: ncl-driver.sh /path/to/ncl}
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

# Run the complete harness in the checkout supplied by the scoreboard runner.
# ncl-driver.lisp loads every pinned benchmark file and calls BENCH-RUN after
# TESTS.LISP has registered the benchmark functions, so the emitted times are
# actual benchmark samples rather than source-load timings.
output=$(mktemp)
trap 'rm -f "$output"' EXIT HUP INT TERM

set +e
"$ncl" --eval "(load \"$script_dir/ncl-driver.lisp\")" >"$output"
status=$?
set -e
if [ "$status" -ne 0 ]; then
    printf '%s\n' '{"status":"failed","times":[]}'
    exit "$status"
fi
cat "$output"
