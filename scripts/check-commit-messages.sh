#!/usr/bin/env bash
#
# check-commit-messages.sh -- commit message policy for this repository.
#
# Rejected:
#   1. Colon prefixes.       "feat: x", "fix(core): x", "chore : x", "Update: x"
#      Conventional Commits is a large-team protocol. This repository writes
#      plain sentences, and mixing the two styles mid-history reads as machine
#      generated.
#   2. Emoji and check marks anywhere in the message.
#   3. Co-authored-by / Signed-off-by trailers, and any mention of the private
#      deploy identity. Author attribution on this repository is a single public
#      identity; a stray trailer links it to unrelated repositories.
#
# Usage:
#   check-commit-messages.sh --range <git-range>   check every commit in a range
#   check-commit-messages.sh --message "<text>"    check one literal message (repeatable)
#   check-commit-messages.sh --stdin               check messages on stdin, one per line
#   check-commit-messages.sh                       check HEAD~20..HEAD, or all history if shorter
#
# Exit status: 0 when every message passes, 1 when any message is rejected.
#
set -uo pipefail

FORBIDDEN_PREFIX='^[A-Za-z]+(\([^)]*\))?[[:space:]]*:'
FORBIDDEN_TRAILER='co-authored-by|signed-off-by|cryptottat'
EMOJI_CLASS='[\x{1F000}-\x{1FAFF}\x{2600}-\x{27BF}\x{FE0F}\x{2705}\x{274C}\x{2714}\x{2716}\x{2B50}]'

FAILED=0
CHECKED=0

check_one() {
  local raw="$1" label="${2:-}"
  local subject
  subject="$(printf '%s' "$raw" | head -1)"
  [ -z "${subject//[[:space:]]/}" ] && return 0
  CHECKED=$((CHECKED + 1))

  local bad=0 why=()

  if printf '%s' "$subject" | grep -qE "$FORBIDDEN_PREFIX"; then
    bad=1
    why+=("colon prefix -- write a plain sentence instead")
  fi
  if printf '%s' "$raw" | grep -qiE "$FORBIDDEN_TRAILER"; then
    bad=1
    why+=("forbidden trailer or identity")
  fi
  if printf '%s' "$raw" | grep -qP "$EMOJI_CLASS"; then
    bad=1
    why+=("emoji or check mark")
  fi

  if [ "$bad" = 1 ]; then
    FAILED=$((FAILED + 1))
    printf 'REJECT %s%s\n' "${label:+$label }" "$subject"
    local r
    for r in "${why[@]}"; do printf '       %s\n' "$r"; done
  else
    printf 'ok     %s%s\n' "${label:+$label }" "$subject"
  fi
}

MODE=range
RANGE=""
declare -a LITERALS=()

while [ $# -gt 0 ]; do
  case "$1" in
    --range)   MODE=range;   RANGE="${2:?--range needs a git range}"; shift 2 ;;
    --message) MODE=literal; LITERALS+=("${2:?--message needs text}"); shift 2 ;;
    --stdin)   MODE=stdin;   shift ;;
    -h|--help) sed -n '2,22p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) printf 'unknown option: %s\n' "$1" >&2; exit 2 ;;
  esac
done

case "$MODE" in
  literal)
    for m in "${LITERALS[@]}"; do check_one "$m"; done
    ;;
  stdin)
    while IFS= read -r line; do check_one "$line"; done
    ;;
  range)
    if [ -z "$RANGE" ]; then
      if git rev-parse --verify -q HEAD~20 >/dev/null 2>&1; then RANGE="HEAD~20..HEAD"; else RANGE="HEAD"; fi
    fi
    if ! git rev-parse --verify -q "${RANGE%%..*}" >/dev/null 2>&1 && [ "$RANGE" != "HEAD" ]; then
      printf 'range %s not resolvable in this repository\n' "$RANGE" >&2
      exit 2
    fi
    while IFS= read -r sha; do
      [ -z "$sha" ] && continue
      body="$(git log -1 --format='%B' "$sha")"
      check_one "$body" "${sha:0:8}"
    done < <(git log --format='%H' "$RANGE" 2>/dev/null)
    ;;
esac

printf '\nchecked %d message(s), %d rejected\n' "$CHECKED" "$FAILED"
[ "$FAILED" -eq 0 ] || exit 1
