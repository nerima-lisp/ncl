#!/bin/sh
set -u

ncl=${1:?usage: ncl-driver.sh /path/to/ncl}
status=0
times=
tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/ncl-cl-bench.XXXXXX")
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM

for entry in \
  "TAK|files/gabriel.lisp" \
  "BOYER|files/gabriel.lisp" \
  "FIB|files/math.lisp" \
  "ACKERMANN|files/math.lisp"; do
  name=${entry%%|*}
  file=${entry#*|}
  time_file="$tmp_dir/$name.time"
  output_file="$tmp_dir/$name.out"
  error_file="$tmp_dir/$name.err"
  /usr/bin/time -p -o "$time_file" "$ncl" --eval \
    "(progn (load \"package.lisp\") (load \"$file\"))" \
    >"$output_file" 2>"$error_file"
  command_status=$?
  elapsed=$(awk '$1 == "real" { print $2 }' "$time_file")
  if [ -z "$elapsed" ]; then
    elapsed=0
  fi
  if [ -n "$times" ]; then
    times="$times,$elapsed"
  else
    times="$elapsed"
  fi
  if [ "$command_status" -ne 0 ]; then
    status=1
  fi
done

printf '{"times":[%s]}\n' "$times"
exit "$status"
