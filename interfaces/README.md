# interfaces/

**Read-only. Export-only.**

This directory is exported from the contract repos (IDLs, event formats, account layouts, golden vectors under `golden/`) by the repo owner, on his own machine. Sessions (human or AI) must never edit it. Any commit touching this path must have a subject starting with `export:` (enforced by `scripts/check_protected_paths.sh` in CI).

Until the first export lands, anything that depends on contents of this directory is marked `TODO(interfaces)` in code and docs.
