#!/bin/sh
set -u

if [ "$#" -ne 2 ]; then
    printf '%s\n' 'usage: ncl-driver.sh /path/to/ncl default|off' >&2
    exit 2
fi

ncl=$1
mode=$2
case "$mode" in
    default|off) ;;
    *)
        printf '%s\n' 'mode must be default or off' >&2
        exit 2
        ;;
esac

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

# Run the complete harness in the checkout supplied by the scoreboard runner.
# ncl-driver.lisp loads every pinned benchmark file and calls BENCH-RUN after
# TESTS.LISP has registered the benchmark functions, so the emitted times are
# actual benchmark samples rather than source-load timings.
output=$(mktemp)
trap 'rm -f "$output"' EXIT HUP INT TERM

set +e
"$ncl" "--optimization=$mode" --eval "(load \"$script_dir/ncl-driver.lisp\")" >"$output"
status=$?
set -e
cat "$output"
exit "$status"
