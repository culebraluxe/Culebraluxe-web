#!/usr/bin/env bash
# Proof for the per-checkout build-dir rule (docs/agent/LAYOUT.md, "One target directory per checkout"), run end
# to end against real probe worktrees. Run it from any checkout:
#
#   bash scripts/verify-checkout-build-dir.sh
#
# A lane proves the sandbox path; the trunk proves the same file covers the checkout the launchd jobs use.
#   1. does git fire .githooks/post-checkout inside a NEW worktree — and is that checkout fast? (this is the guard
#      against the 2026-10-08 regression: the hook's script did work it should not, so a sandbox cost 30s, not 0.6s)
#   2. does the config it writes send the build to <checkout>/.cargo-target?
#   3. is that build dir ignored, so it is never a dirty file?
#   4. what cargo answers WHEN STANDING IN THE SANDBOX — cargo reads its config from the CWD, not from
#      --manifest-path, so this must run with the CWD inside the probe — and what scripts/rust-ui-build.sh:42
#      would make of that answer (its "<checkout>/target" container reading)?
#   5. do two sandboxes get two DIFFERENT build dirs?
#   6. is a checkout named lane-* treated as a lane (build/rust-lane-<name>, outside its tree)?
#   7. is the main checkout REFUSED rather than given a second answer — and is an existing answer left alone?
#   8. post-checkout fires on EVERY branch switch, not only on creation: does re-entry leave the config
#      byte-identical and stay silent (the existence guard is what makes a branch switch cheap)?
# Every check asserts, FAIL sets a non-zero exit, and cleanup runs on the way out (a probe is not a tree: it lives
# and dies inside this command). A probe that only prints is not a receipt — AGENTS.md, "Never say FINISHED
# without the raw output of the verification commands behind it".
#
# It is also the regression guard for the 2026-10-08 bug: .githooks/post-checkout runs the COPY OF THIS SCRIPT
# WHICH LIVES IN THE CHECKOUT IT IS CREATING, so a fix here reaches `git worktree add` only once it is committed.
set -uo pipefail

# $main is the checkout this script was run from; $trunk is the main checkout, git's first listed worktree.
main="$(git -C "$(cd "$(dirname "$0")/.." && pwd -P)" rev-parse --show-toplevel)"
trunk="$(git -C "$main" worktree list --porcelain | sed -n 's/^worktree //p' | head -1)"
[ -d "$trunk/.git" ] || { echo "not a main checkout: $trunk" >&2; exit 1; }
# A lane's answer is hand-named; if this checkout already has one, the probe must leave it alone (check 7).
lane_cfg_before=""
[ -e "$main/.cargo/config.toml" ] && lane_cfg_before="$(sed -n 's/^target-dir = "\(.*\)"$/\1/p' "$main/.cargo/config.toml")"
probe="${TMPDIR:-/tmp}culebraluxe-probe-$$"
probe2="${TMPDIR:-/tmp}culebraluxe-probe2-$$"
laneprobe="${TMPDIR:-/tmp}lane-probe-$$"
fail=0

pass() { echo "  PASS  $1: $2" ; }
assert() { if [ "$2" = "$3" ]; then pass "$1" "$2"; else echo "  FAIL  $1: got [$2] want [$3]"; fail=1; fi ; }
assert_ne() { if [ "$2" != "$3" ]; then pass "$1" "$2"; else echo "  FAIL  $1: got [$2], which must differ from [$3]"; fail=1; fi ; }
assert_nonempty() { if [ -n "$2" ]; then pass "$1" "$2"; else echo "  FAIL  $1: empty"; fail=1; fi ; }

cleanup() {
  for d in "$probe" "$probe2" "$laneprobe"; do
    git -C "$main" worktree remove --force "$d" >/dev/null 2>&1
  done
  git -C "$main" worktree prune
  echo "== 9. cleanup: worktree records $(git -C "$main" worktree list | wc -l | tr -d ' ') (16 = main + 15 lanes)"
}
trap cleanup EXIT

echo "== 1. a new worktree, made the way the engine makes one, timed =="
timed="$( { /usr/bin/time -p git -C "$main" worktree add --detach "$probe" HEAD ; } 2>&1 )"
printf '%s\n' "$timed" | sed -n 's/^\(Preparing\|HEAD is now\|post-checkout\).*/  | &/p'
case "$timed" in *post-checkout:*) pass "hook fired in the new worktree" "yes" ;;
                 *) echo "  FAIL  hook did not fire (no post-checkout line)" ; fail=1 ;; esac
real="$(printf '%s\n' "$timed" | sed -n 's/^real //p')"
if awk -v r="${real:-99}" 'BEGIN{exit !(r < 5)}'; then pass "checkout is fast" "${real}s (< 5s)"
else echo "  FAIL  checkout took ${real}s: the sandbox build-dir write is doing work it should not" ; fail=1 ; fi
probe_real="$(cd "$probe" && pwd -P)"

echo "== 2. the config the hook wrote =="
assert "mode is 600" "$(stat -f '%Lp' "$probe/.cargo/config.toml" 2>/dev/null)" "600"
assert "target-dir" "$(sed -n 's/^target-dir = "\(.*\)"$/\1/p' "$probe/.cargo/config.toml")" "$probe_real/.cargo-target"
cfg_hash="$(shasum -a256 "$probe/.cargo/config.toml" | cut -d' ' -f1)"

echo "== 3. that build dir is ignored (never a dirty file) =="
mkdir -p "$probe/.cargo-target" && touch "$probe/.cargo-target/probe-artifact"
assert_nonempty "ignore rule" "$(git -C "$probe" check-ignore -v .cargo-target/probe-artifact)"
assert "dirty files with a build dir present" "$(git -C "$probe" status --porcelain | wc -l | tr -d ' ')" "0"

echo "== 4. what cargo answers standing in the sandbox =="
resolved="$(cd "$probe" && cargo metadata --no-deps --offline --format-version 1 2>/dev/null \
  | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
assert "cargo target_directory" "$resolved" "$probe_real/.cargo-target"
assert_ne "ui:build would NOT use /target" "$resolved" "/target"
assert_ne "not the main checkout's fallback" "$resolved" "/Users/Shared/dev/build/rust-main"

echo "== 5. two sandboxes, two build dirs =="
git -C "$main" worktree add --detach "$probe2" HEAD >/dev/null 2>&1
d1="$(sed -n 's/^target-dir = "\(.*\)"$/\1/p' "$probe/.cargo/config.toml")"
d2="$(sed -n 's/^target-dir = "\(.*\)"$/\1/p' "$probe2/.cargo/config.toml")"
assert_ne "sandbox one vs sandbox two" "$d1" "$d2"

echo "== 6. a checkout named lane-* is treated as a lane =="
git -C "$main" worktree add --detach "$laneprobe" HEAD >/dev/null 2>&1
assert "lane probe target-dir" "$(sed -n 's/^target-dir = "\(.*\)"$/\1/p' "$laneprobe/.cargo/config.toml")" \
  "/Users/Shared/dev/build/rust-$(basename "$laneprobe")"

echo "== 7. the main checkout is refused, and an existing answer is not rewritten =="
mainout="$(bash "$main/scripts/lane-cargo-config.sh" --at "$trunk" 2>&1)"; mainrc=$?
assert "exit status" "$mainrc" "1"
case "$mainout" in *"is the main checkout"*rust-main*) pass "names the machine fallback" "yes" ;;
                    *) echo "  FAIL  unexpected message: $mainout" ; fail=1 ;; esac
assert "no config written in the trunk" "$([ -e "$trunk/.cargo/config.toml" ] && echo present || echo absent)" "absent"
if [ -n "$lane_cfg_before" ]; then
  assert "this checkout's own answer is unchanged" \
    "$(sed -n 's/^target-dir = "\(.*\)"$/\1/p' "$main/.cargo/config.toml")" "$lane_cfg_before"
else
  pass "no existing answer here to preserve" "skipped: $main has no .cargo/config.toml"
fi

echo "== 8. a branch switch inside the sandbox changes nothing and says nothing =="
switch="$( { /usr/bin/time -p git -C "$probe" checkout --detach HEAD ; } 2>&1 )"
sw="$(printf '%s\n' "$switch" | sed -n 's/^real //p')"
if awk -v r="${sw:-99}" 'BEGIN{exit !(r < 5)}'; then pass "branch switch is fast (existence guard, no subprocess)" "${sw}s (< 5s)"
else echo "  FAIL  branch switch took ${sw}s" ; fail=1 ; fi
case "$switch" in *post-checkout:*) echo "  FAIL  the hook spoke on a branch switch: it should have found the config and stopped" ; fail=1 ;;
                  *) pass "hook stayed silent on the switch" "no output" ;; esac
assert "config byte-identical after re-entry" "$(shasum -a256 "$probe/.cargo/config.toml" | cut -d' ' -f1)" "$cfg_hash"

if [ "$fail" -eq 0 ]; then echo "VERDICT: PASS (every check above)"; else echo "VERDICT: FAIL (see the FAIL lines)"; fi
exit "$fail"
