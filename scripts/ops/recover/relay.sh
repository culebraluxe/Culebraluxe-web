#!/usr/bin/env bash
set -euo pipefail

# RELAY — move a candidate onto main, or refuse it with a reason.
#
#   scripts/ops/recover/relay.sh --candidate origin/cmd-01            # dry run: would it land, and how far behind is it?
#   scripts/ops/recover/relay.sh --candidate origin/cmd-01 --land     # land it: merge, gate it, push, write the receipt
#   scripts/ops/recover/relay.sh --candidate <sha> --base origin/main
#
# WHY THIS EXISTS. A proposal, a lane's branch and a stranded commit are the same problem wearing three hats: work
# that main cannot see, from someone who is not in a position to run the whole gate and push. Before this script the
# answer was "ask the owner", which is why branches sat for five days and 19 commits were lost for hours. The relay
# is the one place that knows how to turn that into a landed commit: it asks git whether the merge is even possible
# BEFORE touching anything, merges in a worktree that was clean and standing on the base, pays T1 on the landed
# result rather than on the claim, pushes through the same hook a human does, and writes the receipt the house
# means by "done". If any step fails, the tree is put back exactly where it was and nothing is pushed.
#
# WHAT IT WILL NOT DO. It will not land a candidate git calls conflicted (that is its owner's rebase, not a merge
# anyone can force), it will not land from a dirty worktree (the merge would eat the dirt), it will not force or
# skip the push hook, and it changes nothing without --land. A dry run is a question, not a transaction.
#
# THE RECEIPT. A landing ends with docs/agent/receipts/relay-<when>-<sha>.md committed to main, and the body of that
# receipt is the gate's own output — not a retelling of it. A claim is not a receipt.

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"

CANDIDATE=""
BASE="${RELAY_BASE:-origin/main}"
LAND=0
RECEIPT_DIR="docs/agent/receipts"
while [ $# -gt 0 ]; do
  case "$1" in
    --candidate)
      [ $# -ge 2 ] || { echo "relay: --candidate needs a ref" >&2; exit 2; }
      CANDIDATE="$2"; shift 2 ;;
    --base)
      [ $# -ge 2 ] || { echo "relay: --base needs a ref" >&2; exit 2; }
      BASE="$2"; shift 2 ;;
    --receipt-dir)
      [ $# -ge 2 ] || { echo "relay: --receipt-dir needs a path" >&2; exit 2; }
      RECEIPT_DIR="$2"; shift 2 ;;
    --land) LAND=1; shift ;;
    -h|--help) sed -n '2,20p' "$0" | sed -e 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "relay: unknown argument '$1' (try --help)" >&2; exit 2 ;;
  esac
done

say() { printf '%s\n' "$*"; }
die() { printf 'relay: %s\n' "$*" >&2; exit 1; }

[ -n "$CANDIDATE" ] || die "--candidate <ref> is required — nothing to relay"

if ! git fetch -q origin 2>/dev/null; then
  say "relay: fetch failed (offline, or no credentials) — relaying against the refs already on this disk"
fi

git rev-parse --verify --quiet "$BASE^{commit}" >/dev/null || die "no such base '$BASE'"
git rev-parse --verify --quiet "$CANDIDATE^{commit}" >/dev/null || die "no such candidate '$CANDIDATE'"
base_sha="$(git rev-parse "$BASE^{commit}")"
cand_sha="$(git rev-parse "$CANDIDATE^{commit}")"
short_base="$(git rev-parse --short "$base_sha")"
short_cand="$(git rev-parse --short "$cand_sha")"

if git merge-base --is-ancestor "$cand_sha" "$base_sha"; then
  say "relay: nothing to do — '$CANDIDATE' ($short_cand) is already inside $BASE ($short_base)."
  exit 0
fi

commits="$(git rev-list --count "$base_sha..$cand_sha")"
behind="$(git rev-list --count "$cand_sha..$base_sha")"
merge_base="$(git merge-base "$base_sha" "$cand_sha")"
files="$(git diff --name-only "$merge_base..$cand_sha" | wc -l | tr -d ' ')"

say "relay  candidate=$CANDIDATE ($short_cand)   base=$BASE ($short_base)"
say "  commits to land   $commits"
say "  never seen trunk  $behind commits of $BASE this candidate was written before"
say "  touches           $files file(s) against the merge base ($(git rev-parse --short "$merge_base"))"

# Feasibility, asked of git and of nothing else: merge-tree writes the merged tree into the object database without
# checking anything out, so a dry run costs nothing and cannot dirty a worktree.
mt_out="$(mktemp -t relay-mergetree)"
clean="unknown"
if git merge-tree --write-tree --name-only "$base_sha" "$cand_sha" >"$mt_out" 2>&1; then
  clean="yes"
elif grep -qi 'usage:' "$mt_out"; then
  clean="unknown"
else
  clean="no"
fi

case "$clean" in
  yes) say "  feasibility       clean — git merges it without a conflict" ;;
  no)
    say "  feasibility       CONFLICT — relay cannot land this; its owner has to rebase it first"
    sed -n '2,10p' "$mt_out" | sed 's/^/      /'
    say ""
    say "relay: refusing — landing this would look like a merge and behave like an argument. Nothing was changed."
    exit 1
    ;;
  unknown) say "  feasibility       unchecked — this git predates 'merge-tree --write-tree' (2.38), so the dry run cannot know" ;;
esac

if [ "$LAND" != "1" ]; then
  say ""
  say "relay: dry run — nothing was changed. With --land it would merge, run T1 on the landed result, push main, and"
  say "       commit docs/agent/receipts/relay-<when>-$short_cand.md. Any failure puts this worktree back on $short_base."
  exit 0
fi

# --- The transaction. Preconditions first: the relay merges INTO this worktree, so dirt here would be eaten by the
# --- merge and by the abort path. That is a refusal with instructions, never a warning.
dirty="$(git status --porcelain | wc -l | tr -d ' ')"
if [ "$dirty" != "0" ]; then
  die "refusing to land: $dirty uncommitted path(s) in this worktree ($(basename "$PWD")). Land from a clean one, or commit/stash first — a merge would eat them."
fi
if [ "$(git rev-parse HEAD)" != "$base_sha" ]; then
  die "refusing to land: HEAD is $(git rev-parse --short HEAD), not $BASE ($short_base). This worktree has to be standing on the base so that a failed landing can be undone exactly (git switch --detach $BASE keeps any commits under a branch name)."
fi

say ""
say "relay: merging $short_cand into this worktree (no commit yet)..."
if ! git merge --no-ff --no-edit -m "relay: land $CANDIDATE ($short_cand) onto $BASE" "$cand_sha" >/dev/null 2>&1; then
  git merge --abort 2>/dev/null || git reset --hard "$base_sha" >/dev/null
  die "the merge failed even though git called it clean — this worktree is back on $short_base and nothing was pushed"
fi
merged_sha="$(git rev-parse HEAD)"

gate_receipt="$(mktemp -t relay-gate)"
say "relay: T1 on the landed result (slice-check --since $short_base — the gate runs on the merge, not on the claim)..."
echo
if ! bash "$ROOT/scripts/ops/gate/slice-check.sh" --since "$base_sha" --receipt "$gate_receipt"; then
  git reset --hard "$base_sha" >/dev/null
  echo
  die "T1 failed — the merge was undone, nothing was pushed, and this worktree is back on $short_base. The merge commit $(git rev-parse --short "$merged_sha") still exists in the object database if you want to look at it. The failure above is the candidate's to fix, not the relay's."
fi

echo
say "relay: pushing to main — the same pre-push hook a human meets..."
if ! git push origin HEAD:main; then
  git reset --hard "$base_sha" >/dev/null
  die "the push was refused — the merge was undone and this worktree is back on $short_base. Fix what the hook named, then relay again."
fi
landed_sha="$(git rev-parse HEAD)"
short_landed="$(git rev-parse --short HEAD)"
say "relay: main is now $short_landed (the merge commit is $landed_sha)"

# --- The receipt: the gate's own words, committed, so the next reader does not have to take anyone's word for it.
mkdir -p "$RECEIPT_DIR"
receipt="$RECEIPT_DIR/relay-$(date -u +%Y%m%d-%H%M%S)-$short_cand.md"
{
  printf '# relay receipt — %s\n\n' "$CANDIDATE"
  cat <<EOF
- landed: \`$short_cand\` onto \`main\` as \`$short_landed\`
- base: \`$BASE\` at \`$short_base\`
- commits landed: $commits
- candidate was written $behind commit(s) before \`$BASE\`
- when: $(date -u '+%Y-%m-%d %H:%M:%SZ')
- by: relay.sh (scripts/ops/recover/relay.sh)

## The gate this landing paid

\`\`\`
$(cat "$gate_receipt")
\`\`\`
EOF
} > "$receipt"
if git add "$receipt" && git commit -q -m "relay: receipt for landing $CANDIDATE ($short_cand)" && git push -q origin HEAD:main; then
  say "relay: receipt committed: $receipt"
else
  say "relay: the landing SUCCEEDED (main is at $short_landed) but the receipt did not push."
  say "relay: the receipt is written at $receipt — commit and push it by hand so the landing is not a claim."
  exit 1
fi

echo
say "relay: landed $CANDIDATE as $short_landed. This worktree is detached at it, which is the same commit as $BASE now."
exit 0
