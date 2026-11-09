"""Pinned benchmark paths, limits, and benchmark names."""

from pathlib import Path

from benchmark_constants import REPO_ROOT, SBCL_VERSION


NCL_DRIVER = REPO_ROOT / "conformance/cl-bench/ncl-driver.sh"
SBCL_DRIVER = REPO_ROOT / "conformance/cl-bench/sbcl-driver.lisp"
DRIVER_SUPERVISOR = REPO_ROOT / "conformance/cl-bench/driver_supervisor.py"
INFO_TIMEOUT = 30.0
MAX_RUNNER_OUTPUT_CHARS = 1_000_000
MAX_BENCHMARK_TIMES = 10_000
MAX_FLAT_TIMES = 650_000
MAX_ERROR_CHARS = 64_000

BENCHMARKS = (
    "1D-ARRAYS",
    "2D-ARRAYS",
    "3D-ARRAYS",
    "ACKERMANN",
    "BENCH-STRINGS",
    "BIGNUM/ELEM-100-1000",
    "BIGNUM/ELEM-1000-100",
    "BIGNUM/ELEM-10000-1",
    "BIGNUM/PARI-100-10",
    "BIGNUM/PARI-200-5",
    "BITVECTORS",
    "BOEHM-GC",
    "BOYER",
    "BROWSE",
    "CLOS/complex-methods",
    "CLOS/defclass",
    "CLOS/defmethod",
    "CLOS/instantiate",
    "CLOS/method+after",
    "CLOS/methodcalls",
    "CLOS/simple-instantiate",
    "COMPILER",
    "CRC40",
    "CTAK",
    "DDERIV",
    "DEFLATE-FILE",
    "DERIV",
    "DESTRUCTIVE",
    "DIV2-TEST-1",
    "DIV2-TEST-2",
    "EQL-SPECIALIZED-FIB",
    "FACTORIAL",
    "FFT",
    "FIB",
    "FIB-RATIO",
    "FPRINT/PRETTY",
    "FPRINT/UGLY",
    "FRPOLY/BIGNUM",
    "FRPOLY/FIXNUM",
    "FRPOLY/FLOAT",
    "HASH-INTEGERS",
    "HASH-STRINGS",
    "LOAD-FASL",
    "MANDELBROT/COMPLEX",
    "MANDELBROT/DFLOAT",
    "MRG32K3A",
    "PI-ATAN",
    "PI-DECIMAL/BIG",
    "PI-DECIMAL/SMALL",
    "PI-RATIOS",
    "PUZZLE",
    "RICHARDS",
    "SEARCH-SEQUENCE",
    "SLURP-LINES",
    "STAK",
    "STRING-CONCAT",
    "SUM-PERMUTATIONS",
    "TAK",
    "TAKL",
    "TRAVERSE",
    "TRIANGLE",
    "TRTAK",
    "WALK-LIST/MESS",
    "WALK-LIST/SEQ",
    "fill-strings/adjustable",
)
DISABLED_BENCHMARK = "WALK-LIST/MESS"
DISABLED_BENCHMARK_REASONS = {
    DISABLED_BENCHMARK: (
        "The pinned cl-bench definition lists SBCL under disabled-for for "
        "WALK-LIST/MESS."
    ),
}
RUNNER_STATUSES = {"passed", "disabled", "failed"}
