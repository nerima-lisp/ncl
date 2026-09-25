# Conformance libraries

The library-load conformance matrix is fixed to the following 14 libraries.
The first ten entries use the Quicklisp distribution identifier recorded by
the source snapshot. The last four entries use the exact source commit used
for the additional clone. These references are part of the conformance
contract and must not drift without updating the plan and its acceptance
results.

| library | fixed revision or commit | source reference |
| --- | --- | --- |
| cffi | `20260101-git` | Quicklisp distribution |
| bordeaux-threads | `v0.9.4` | Quicklisp distribution |
| closer-mop | `20260101-git` | Quicklisp distribution |
| trivial-features | `20250622-git` | Quicklisp distribution |
| trivial-garbage | `20231021-git` | Quicklisp distribution |
| trivial-gray-streams | `20241012-git` | Quicklisp distribution |
| trivial-backtrace | `20230214-git` | Quicklisp distribution |
| alexandria | `20241012-git` | Quicklisp distribution |
| cl-ppcre | `20250622-git` | Quicklisp distribution |
| babel | `20260101-git` | Quicklisp distribution |
| usocket | `4951d575b8f73270802a03cc5812b8310409caa9` | additional clone |
| flexi-streams | `3d9d89b4950b72e0e5bdacfcdfd366bde72386d2` | additional clone |
| static-vectors | `d492f746d33fed32283143394c7ead1cce59ba37` | additional clone |
| cl-fad | `714257f064cbe326855701be1aa5ef1199f3c676` | additional clone |

The test uses each library's upstream source together with an NCL backend
under `libraries/<name>/` when one is needed. Which libraries need a backend
is determined by the library-load work, not by this pin list.

For provenance, the Quicklisp identifiers above were recorded from the
Quicklisp source distributions. The four additional clone commits are
`usocket` `4951d575b8f73270802a03cc5812b8310409caa9`, `flexi-streams`
`3d9d89b4950b72e0e5bdacfcdfd366bde72386d2`, `static-vectors`
`d492f746d33fed32283143394c7ead1cce59ba37`, and `cl-fad`
`714257f064cbe326855701be1aa5ef1199f3c676`.

UIOP is tested separately from the 14-library matrix. Its source is the
`uiop.lisp` bundled with SBCL 2.6.0's ASDF; it was not a git-managed
Quicklisp source, so the bundled source is the recorded provenance.
