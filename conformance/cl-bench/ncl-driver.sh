#!/bin/sh
set -u

if [ "$#" -lt 2 ]; then
    printf '%s\n' 'usage: ncl-driver.sh /path/to/ncl default|off [benchmark ...]' >&2
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
shift 2
selection=
if [ "$#" -gt 0 ]; then
    selection="("
    for benchmark in "$@"; do
        case "$benchmark" in
            ''|*[!a-zA-Z0-9/-]*)
                printf '%s\n' 'benchmark names may contain only letters, digits, slash, or hyphen' >&2
                exit 2
                ;;
        esac
        selection="$selection \"$benchmark\""
    done
    selection="$selection )"
fi

# Run the complete harness in the checkout supplied by the scoreboard runner.
# ncl-driver.lisp loads every pinned benchmark file and calls BENCH-RUN after
# TESTS.LISP has registered the benchmark functions, so the emitted times are
# actual benchmark samples rather than source-load timings.
output=$(mktemp)
trap 'rm -f "$output"' EXIT HUP INT TERM

set +e
if [ -n "$selection" ]; then
    form="(progn (setf *cl-bench-selection* '$selection) (load \"$script_dir/ncl-driver.lisp\"))"
else
    form="(load \"$script_dir/ncl-driver.lisp\")"
fi
"$ncl" "--optimization=$mode" --eval "$form" >"$output"
status=$?
set -e
cat "$output"
exit "$status"
