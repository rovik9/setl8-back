# vendor/

**Read-only. Export-only.**

This directory holds the pure integer engine and evaluator crates exported from the sector contract repos, by the repo owner, on his own machine. There is no second implementation of the rules in this repo. Sessions (human or AI) must never edit it. Any commit touching this path must have a subject starting with `export:` (enforced by `scripts/check_protected_paths.sh` in CI).
