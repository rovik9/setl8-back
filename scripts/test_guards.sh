#!/usr/bin/env bash
# Self-test for check_identity.sh and check_protected_paths.sh.
# Builds throwaway repos in a temp dir and asserts that each guard passes on
# good history and fails on bad history. Uses only fixture identities.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
fails=0

GOOD_NAME="Rovik"
GOOD_EMAIL="therovikdev@gmail.com"

new_repo() {
  rm -rf "$tmp/r" && mkdir "$tmp/r" && cd "$tmp/r"
  git init -q -b main
  git config commit.gpgsign false
  commit_as "$GOOD_NAME" "$GOOD_EMAIL" "$GOOD_NAME" "$GOOD_EMAIL" "base" base.txt
  git tag base
}

# commit_as AUTHOR_NAME AUTHOR_EMAIL COMMITTER_NAME COMMITTER_EMAIL SUBJECT FILE [BODY]
commit_as() {
  mkdir -p "$(dirname "$6")"
  echo "$RANDOM$RANDOM" >> "$6"
  git add "$6"
  msg="$5"
  if [ -n "${7:-}" ]; then msg="$5

$7"; fi
  GIT_AUTHOR_NAME="$1" GIT_AUTHOR_EMAIL="$2" GIT_COMMITTER_NAME="$3" GIT_COMMITTER_EMAIL="$4" \
    git commit -q -m "$msg"
}

expect() { # expect pass|fail DESCRIPTION CMD...
  want="$1"; desc="$2"; shift 2
  if "$@" >/dev/null 2>&1; then got=pass; else got=fail; fi
  if [ "$got" = "$want" ]; then echo "ok   - $desc"; else echo "FAIL - $desc (wanted $want, got $got)"; fails=$((fails + 1)); fi
}

ident="$here/check_identity.sh"
prot="$here/check_protected_paths.sh"

new_repo
commit_as "$GOOD_NAME" "$GOOD_EMAIL" "$GOOD_NAME" "$GOOD_EMAIL" "feat: ok" a.txt
expect pass "identity: clean history passes" "$ident" base..HEAD

new_repo
commit_as "Someone" "x@example.com" "$GOOD_NAME" "$GOOD_EMAIL" "bad author" a.txt
expect fail "identity: wrong author fails" "$ident" base..HEAD

new_repo
commit_as "$GOOD_NAME" "$GOOD_EMAIL" "GitHub" "noreply@github.com" "bad committer" a.txt
expect fail "identity: wrong committer fails" "$ident" base..HEAD

new_repo
commit_as "$GOOD_NAME" "$GOOD_EMAIL" "$GOOD_NAME" "$GOOD_EMAIL" "trailer" a.txt "Co-Authored-By: X <x@example.com>"
expect fail "identity: Co-Authored-By trailer fails" "$ident" base..HEAD

new_repo
commit_as "$GOOD_NAME" "$GOOD_EMAIL" "$GOOD_NAME" "$GOOD_EMAIL" "trailer" a.txt "claude-session: https://example.invalid/s"
expect fail "identity: session-link trailer fails (case-insensitive)" "$ident" base..HEAD

new_repo
commit_as "$GOOD_NAME" "$GOOD_EMAIL" "$GOOD_NAME" "$GOOD_EMAIL" "feat: ok" a.txt
commit_as "Someone" "x@example.com" "$GOOD_NAME" "$GOOD_EMAIL" "bad in middle" b.txt
commit_as "$GOOD_NAME" "$GOOD_EMAIL" "$GOOD_NAME" "$GOOD_EMAIL" "feat: ok2" c.txt
expect fail "identity: one bad commit among good ones fails" "$ident" base..HEAD

new_repo
commit_as "$GOOD_NAME" "$GOOD_EMAIL" "$GOOD_NAME" "$GOOD_EMAIL" "feat: unrelated" src/a.txt
expect pass "protected: unrelated change passes" "$prot" base..HEAD

new_repo
commit_as "$GOOD_NAME" "$GOOD_EMAIL" "$GOOD_NAME" "$GOOD_EMAIL" "export: add vector" interfaces/golden/v.txt
commit_as "$GOOD_NAME" "$GOOD_EMAIL" "$GOOD_NAME" "$GOOD_EMAIL" "export: bump engine" vendor/engine/lib.txt
expect pass "protected: export: commits pass" "$prot" base..HEAD

new_repo
commit_as "$GOOD_NAME" "$GOOD_EMAIL" "$GOOD_NAME" "$GOOD_EMAIL" "tweak interfaces" interfaces/x.txt
expect fail "protected: interfaces/ without export: fails" "$prot" base..HEAD

new_repo
commit_as "$GOOD_NAME" "$GOOD_EMAIL" "$GOOD_NAME" "$GOOD_EMAIL" "tweak vendor" vendor/x.txt
expect fail "protected: vendor/ without export: fails" "$prot" base..HEAD

new_repo
commit_as "$GOOD_NAME" "$GOOD_EMAIL" "$GOOD_NAME" "$GOOD_EMAIL" "export: ok" interfaces/x.txt
commit_as "$GOOD_NAME" "$GOOD_EMAIL" "$GOOD_NAME" "$GOOD_EMAIL" "sneaky edit" vendor/y.txt
expect fail "protected: one bad commit among exports fails" "$prot" base..HEAD

[ "$fails" -eq 0 ] && echo "all guard tests passed"
exit "$fails"
