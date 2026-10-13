#!/usr/bin/env bash
# Usage: check_identity.sh <rev-range>   e.g. origin/main..HEAD
# Every non-merge commit must have author and committer exactly
# "Rovik <therovikdev@gmail.com>", and no message may contain
# "Co-Authored-By" or "Claude-Session" (case-insensitive).
set -euo pipefail
range="${1:?usage: check_identity.sh <rev-range>}"
want="Rovik <therovikdev@gmail.com>"
fail=0
for c in $(git rev-list "$range"); do
  author=$(git show -s --format='%an <%ae>' "$c")
  committer=$(git show -s --format='%cn <%ce>' "$c")
  if [ "$author" != "$want" ]; then echo "BAD author in $c: $author"; fail=1; fi
  if [ "$committer" != "$want" ]; then echo "BAD committer in $c: $committer"; fail=1; fi
  if git show -s --format='%B' "$c" | grep -qiE 'co-authored-by|claude-session'; then
    echo "BAD trailer in $c"; fail=1
  fi
done
[ "$fail" -eq 0 ] && echo "identity check OK ($range)"
exit "$fail"
