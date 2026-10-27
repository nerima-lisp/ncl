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

for benchmark in "$@"; do
    case "$benchmark" in
        ''|*[!a-zA-Z0-9/-]*)
            printf '%s\n' 'benchmark names may contain only letters, digits, slash, or hyphen' >&2
            exit 2
            ;;
    esac
done

exec python3 "$script_dir/driver_supervisor.py" \
    --engine ncl \
    --runner "$ncl" \
    --mode "$mode" \
    --driver "$script_dir/ncl-driver.lisp" \
    --cwd "$PWD" \
    "$@"
