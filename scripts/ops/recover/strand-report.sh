#!/usr/bin/env bash
set -euo pipefail

# STRAND REPORT — every commit the house cannot see from `origin/main`.
#
#   scripts/ops/recover/strand-report.sh                report; always exits 0
#   scripts/ops/recover/strand-report.sh --fail-over 0  exits 1 if anything is stranded (cron / gate use)
#
# WHY THIS EXISTS. On 2026-10-02 a session crashed holding 19 commits on a branch. The branch was on origin the
# whole time — pullable, reviewable, deployable — and it was still lost for hours, because the House Rules had
# taught every agent that branches are not a place where work lives. A prohibition that everyone breaks does not
# stop branching; it makes branching invisible. This report answers "what exists that main cannot see", and it is
# cheap enough to run before you start work, before you stop, and from cron.
#
# A STRANDED BRANCH IS A ROW. Nothing is hidden here for looking abandoned or ancient: age and distance from main
# are the two facts a reader needs in order to decide, so they are printed, never filtered. A branch with zero
# commits missing from main is not stranded and is not printed.
#
# READ-ONLY. It fetches refs, and it reads other lanes' worktrees with `--no-optional-locks` so it cannot race a
# lane that is mid-commit. It commits nothing, deletes nothing, and never writes to a working tree.
#
# THE REST OF THE SUITE, in the order it should exist (none of it exists yet): `land` (the landing transaction,
# with a receipt per landing), `lock-guard` (manifest/lock drift — and whose drift it is), `gate-baseline`
# (per-gate ratchets: fmt, the 800-line rule, advisories), `triage-ledger-check`, `handoff-journal`, `main-pulse`,
# `preflight`.

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"

REMOTE="${STRAND_REMOTE:-origin}"
BASE="${STRAND_BASE:-$REMOTE/main}"
FAIL_OVER=""
if [ "${1:-}" = "--fail-over" ]; then FAIL_OVER="${2:-0}"; fi

# Sibling worktrees are legitimate — `Culebraluxe-web`, `-claude`, `-shell` are where the lanes live — so the
# report only raises a flag for a worktree outside the lane root, or one sitting in a temp directory (macOS
# clears those without warning, and a lane's work inside one is gone when it does).
MAIN_WT="$(git worktree list --porcelain 2>/dev/null | sed -n '1s/^worktree //p')"
LANE_ROOT="$(dirname "${MAIN_WT:-$ROOT}")"

NOW="$(date -u +%s)"
age_of() { # $1 = committer epoch
  local days=$(( (NOW - $1) / 86400 ))
  if [ "$days" -le 0 ]; then printf 'today'; else printf '%sd' "$days"; fi
}
clip() { # $1 = string, clipped so a row stays one line
  local s="$1"
  if [ "${#s}" -gt 64 ]; then printf '%s...' "${s:0:61}"; else printf '%s' "$s"; fi
}
short_path() { # $1 = path, with $HOME shown as ~
  local p="$1"
  case "$p" in "$HOME"/*) p="~/${p#"$HOME"/}" ;; esac
  if [ "${#p}" -gt 54 ]; then printf '%s...' "${p:0:51}"; else printf '%s' "$p"; fi
}

FETCH="fetched"
if ! git fetch -q "$REMOTE" 2>/dev/null; then
  FETCH="NOT FETCHED (offline, or no credentials) — counts are against your last fetch"
fi
if ! git rev-parse --verify --quiet "$BASE" >/dev/null; then
  echo "strand-report: no $BASE to compare against. Set STRAND_BASE or check the remote." >&2
  exit 2
fi

printf 'strand-report  base=%s@%s  %s\n' "$BASE" "$(git rev-parse --short "$BASE")" "$FETCH"

remote_stranded=0
rows=""
while read -r b; do
  case "$b" in main|HEAD|'') continue ;; esac
  ahead="$(git rev-list --count "$BASE".."$REMOTE/$b" 2>/dev/null || echo 0)"
  if [ "$ahead" -gt 0 ]; then
    behind="$(git rev-list --count "$REMOTE/$b".."$BASE" 2>/dev/null || echo 0)"
    tip="$(git log -1 --format='%ct|%ae|%s' "$REMOTE/$b" 2>/dev/null || true)"
    stamp="${tip%%|*}"; rest="${tip#*|}"; who="${rest%%|*}"; subject="${rest#*|}"
    rows="${rows}$(printf '%-48s %6s %7s %6s  %-24s %s' "$b" "$ahead" "$behind" "$(age_of "$stamp")" "$who" "$(clip "$subject")")"$'\n'
    remote_stranded=$((remote_stranded + 1))
  fi
done < <(git for-each-ref --format='%(refname:short)' "refs/remotes/$REMOTE" | sed "s|^$REMOTE/||")

echo
printf '%s\n' "=== on $REMOTE, carrying commits main cannot see"
if [ -n "$rows" ]; then
  printf '%-48s %6s %7s %6s  %-24s %s\n' 'BRANCH' 'AHEAD' 'BEHIND' 'AGE' 'WHO' 'TIP'
  printf '%s' "$rows" | sort -k2,2nr
else
  echo "  (none)"
fi

local_only=0
unpushed=0
lrows=""
while read -r b; do
  [ "$b" = "main" ] && continue
  ahead="$(git rev-list --count "$BASE".."$b" 2>/dev/null || echo 0)"
  if [ "$ahead" -gt 0 ]; then
    if git show-ref --verify --quiet "refs/remotes/$REMOTE/$b"; then
      up="$(git rev-list --count "refs/remotes/$REMOTE/$b".."$b" 2>/dev/null || echo 0)"
      state="on $REMOTE ($up unpushed)"
      unpushed=$((unpushed + up))
    else
      state="LOCAL ONLY - nobody else can see it"
      local_only=$((local_only + 1))
    fi
    tip="$(git log -1 --format='%ct|%s' "$b" 2>/dev/null || true)"
    stamp="${tip%%|*}"; subject="${tip#*|}"
    lrows="${lrows}$(printf '%-48s %6s %6s  %-34s %s' "$b" "$ahead" "$(age_of "$stamp")" "$state" "$(clip "$subject")")"$'\n'
  fi
done < <(git for-each-ref --format='%(refname:short)' refs/heads)

echo
printf '%s\n' "=== in this clone, not yet on $REMOTE"
if [ -n "$lrows" ]; then
  printf '%-48s %6s %6s  %-34s %s\n' 'BRANCH' 'AHEAD' 'AGE' 'WHERE' 'TIP'
  printf '%s' "$lrows" | sort -k2,2nr
else
  echo "  (none)"
fi

detached=0
dirty=0
outside=0
temp=0
wrows=""
while IFS= read -r line; do
  case "$line" in
    worktree\ *) wt_path="${line#worktree }"; wt_ref=""; wt_head="" ;;
    HEAD\ *) wt_head="${line#HEAD }" ;;
    branch\ *) wt_ref="${line#branch refs/heads/}" ;;
    detached) wt_ref="(detached HEAD)" ;;
    '')
      if [ -n "${wt_path:-}" ]; then
        files="$(git --no-optional-locks -C "$wt_path" status --porcelain 2>/dev/null | wc -l | tr -d ' ')"
        flags=""
        if [ "$wt_ref" = "(detached HEAD)" ]; then flags="detached"; detached=$((detached + 1)); fi
        if [ "${files:-0}" -gt 0 ]; then
          flags="${flags:+$flags, }$files uncommitted"
          dirty=$((dirty + 1))
        fi
        case "$wt_path" in
          "$LANE_ROOT"|"$LANE_ROOT"/*) ;;
          *) flags="${flags:+$flags, }OUTSIDE THE LANE ROOT"; outside=$((outside + 1)) ;;
        esac
        case "$wt_path" in
          /tmp/*|/private/tmp/*|"${TMPDIR:-/var/empty}"*) flags="${flags:+$flags, }IN A TEMP DIR"; temp=$((temp + 1)) ;;
        esac
        wrows="${wrows}$(printf '%-56s %-9s %-22s %s' "$(short_path "$wt_path")" "$(printf '%.9s' "${wt_head:-?}")" "$wt_ref" "$flags")"$'\n'
        wt_path=""
      fi
      ;;
  esac
done < <(git worktree list --porcelain 2>/dev/null; echo)

echo
printf '%s\n' '=== worktrees on this machine'
if [ -n "$wrows" ]; then
  printf '%-56s %-9s %-22s %s\n' 'PATH' 'HEAD' 'BRANCH' 'STATE'
  printf '%s' "$wrows"
else
  echo "  (none)"
fi

echo
printf 'stranded on %s: %s branch(es) | local only: %s | unpushed commits: %s | detached worktrees: %s | dirty worktrees: %s | outside the lane root: %s | in a temp dir: %s\n' \
  "$REMOTE" "$remote_stranded" "$local_only" "$unpushed" "$detached" "$dirty" "$outside" "$temp"

if [ "$local_only" -gt 0 ] || [ "$temp" -gt 0 ]; then
  printf '%s\n' 'ATTENTION: work above exists in exactly one place. It is not a branch anyone can pull.'
fi
printf '%s\n' 'Next: land what should live (rebase onto origin/main, then push origin lane/<name>:main), or delete what should not.'

if [ -n "$FAIL_OVER" ]; then
  total=$((remote_stranded + local_only))
  if [ "$total" -gt "$FAIL_OVER" ]; then
    printf 'strand-report: %s stranded (over the --fail-over %s limit)\n' "$total" "$FAIL_OVER"
    exit 1
  fi
fi
exit 0
