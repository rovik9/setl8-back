#!/usr/bin/env bash
# Usage: check_protected_paths.sh <rev-range>
# Every commit in the range touching interfaces/ or vendor/ must have a
# subject starting with "export:".
set -euo pipefail
range="${1:?usage: check_protected_paths.sh <rev-range>}"
fail=0
for c in $(git rev-list --no-merges "$range" -- interfaces vendor); do
  subject=$(git show -s --format='%s' "$c")
  case "$subject" in
    export:*) ;;
    *) echo "commit $c touches interfaces/ or vendor/ without 'export:' subject: $subject"; fail=1 ;;
  esac
done
[ "$fail" -eq 0 ] && echo "protected paths check OK ($range)"
exit "$fail"
