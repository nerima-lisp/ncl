#!/bin/sh
set -u

ncl=${1:?usage: ncl-driver.sh /path/to/ncl}
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

# Run the complete harness in the checkout supplied by the scoreboard runner.
# ncl-driver.lisp loads every pinned benchmark file and calls BENCH-RUN after
# TESTS.LISP has registered the benchmark functions, so the emitted times are
# actual benchmark samples rather than source-load timings.
exec "$ncl" --eval "(load \"$script_dir/ncl-driver.lisp\")"
