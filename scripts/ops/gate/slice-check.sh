#!/usr/bin/env bash
# The T1 gate for one slice (AGENTS.md, "The gate is tiered").
#
# A slice owes: it compiles (T0), the sections it touched pass (T1), and rustfmt is happy with the trees it touched. It
# does NOT owe the whole suite (T2) — that is 1062 tests, and it runs on main, on the nightly/Jenkins run and at release.
# This script is the T1 half, so a hand-off carries a receipt instead of a claim, and so the author's window is spent
# writing code rather than waiting for tests that are not about his change.
#
#   pnpm slice:check                   # the working tree, or every commit since the merge-base with origin/main
#   pnpm slice:check --since <ref>     # a committed slice against a named base (e.g. the sha an author was stamped)
#   pnpm slice:check --full            # also run T2 here (cargo nextest --workspace --profile ci, else cargo test)
#   pnpm slice:check --receipt <path>  # write the same block it prints, for PROPOSAL.md or RECEIPT.md
#
# Exit 0 = the slice may be handed over. Exit 1 = it may not, and the failing stage is named.
#
# Why T1 and not "everything, to be safe": scope is the whole point of the tier. Widening it back to the workspace is
# what burned the windows this script protects; narrowing it to the green files is fraud. Run what you touched — the
# sections' crates, including the tests you broke. `forge test-section --changed` (cli/src/forge/test_section.rs) is
# the house's own logical domains and the only taxonomy this script trusts.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"

usage() { sed -n '2,19p' "$0" | sed -e 's/^# \{0,1\}//'; }

since=""
full=0
receipt=""
while [ $# -gt 0 ]; do
  case "$1" in
    --since|--receipt)
      flag="$1"
      if [ $# -lt 2 ]; then
        echo "slice-check: $flag needs a value" >&2
        exit 2
      fi
      value="$2"
      shift 2
      case "$flag" in
        --since) since="$value" ;;
        --receipt) receipt="$value" ;;
      esac
      ;;
    --full)
      full=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "slice-check: unknown argument '$1' (try --help)" >&2
      exit 2
      ;;
  esac
done

# What is under test. `test-section --changed` reads the working tree when given no --since, which would hide a slice
# that is already committed — so choose the mode deliberately and say which one ran, rather than reporting a green
# "nothing changed".
mode=""
include_dirty=0
if [ -n "$since" ]; then
  mode="committed slice since $since"
else
  if [ "$(git status --porcelain | wc -l | tr -d ' ')" != "0" ]; then
    mode="working tree (uncommitted)"
    include_dirty=1
  elif git rev-parse --verify --quiet origin/main >/dev/null 2>&1; then
    base="$(git merge-base origin/main HEAD)"
    if [ "$base" != "$(git rev-parse HEAD)" ]; then
      since="$base"
      mode="committed slice since $base (merge-base with origin/main)"
    fi
  fi
fi

if [ -z "$mode" ]; then
  echo "slice-check: nothing changed since origin/main and the working tree is clean — nothing to check."
  exit 0
fi

# The changed paths, computed here rather than parsed back out of the taxonomy: the receipt wants the count, and rustfmt
# needs to know which files are *this slice's* — see FMT below, where main's pre-existing dirt is reported instead of
# charged.
changed_files=""
if [ "$include_dirty" = "1" ]; then
  changed_files="$(git status --porcelain | cut -c4- | grep -v ' -> ' || true)"
else
  changed_files="$(git diff --name-only "$since"...HEAD | grep -v ' -> ' || true)"
fi
changed_count="$(printf '%s\n' "$changed_files" | grep -c . || true)"

log="$(mktemp -t slice-check)"
trap 'rm -f "$log"' EXIT

t0="-"
fmt="-"
t1="-"
t2="NOT RUN — CI on push, the nightly run and releases own T2 (--full pays for it here)"
t2name="cargo nextest run --workspace --profile ci"
sections=""
crates_line=""
failed=""

# --- T0 — it compiles -------------------------------------------------------------------------------------------
# The same check the pre-push hook runs, so the receipt says something the push will agree with. Warm target dir:
# seconds. Cold: the price of the first build, paid once per machine.
echo "slice-check: T0 — cargo check --workspace --all-targets"
started=$SECONDS
if cargo check --manifest-path Cargo.toml --workspace --all-targets --quiet >"$log" 2>&1; then
  t0="PASS ($((SECONDS - started))s)"
else
  t0="FAIL ($((SECONDS - started))s)"
  failed="T0 — the workspace does not compile"
  cat "$log" >&2
fi

# --- FMT — rustfmt, scoped to this slice's files, with the deferrals CI honours and no others ---------------------
# Two scopes, both deliberate. The four deferred trees are exempt because CI exempts them (the same regex, copied, not
# re-guessed). And a file this slice did NOT touch may be unformatted without failing it. rustfmt is clean on `main`
# today — this scope is insurance, not a workaround: the standing reds are `osv-scanner` and the 800-line boundedness
# rule, and the day a formatting drift joins them it belongs to the ratchet, not to every slice. A slice that cannot
# land because of dirt it did not create is the pathology this tier exists to end. Reported, never hidden: a receipt
# that says PASS while listing someone else's unformatted file is honest; one that says PASS and says nothing is not.
if [ -z "$failed" ]; then
  echo "slice-check: FMT — rustfmt on this slice's files (four deferred trees exempt: the gates.yml list, not a guess)"
  started=$SECONDS
  if ! cargo fmt --version >/dev/null 2>&1; then
    fmt="SKIPPED (no rustfmt on PATH)"
  else
    deferred='^Diff in .*/(cli/src/(apple_mail|forge/lint)|core/domain/src/(applemail|apple_messages))(\.rs|/)'
    fmt_raw="$( cargo fmt --all -- --check 2>&1 || true )"
    # `cargo fmt --check` prints each offending file's whole diff, body and all: only the `Diff in <path> at line N:`
    # headers name files, so the body is context, not error output. Treating the body as "something I do not understand"
    # is exactly how the first run of this script reported UNKNOWN (2026-10-02).
    unformatted="$(printf '%s\n' "$fmt_raw" | grep '^Diff in' | grep -Ev "$deferred" || true)"
    mine=""
    theirs=""
    while IFS= read -r diff_line; do
      [ -n "$diff_line" ] || continue
      rel="${diff_line#Diff in }"
      # rustfmt names the offending file two ways: `Diff in <path> at line N:` -- the shape this split was written
      # against -- and `Diff in <path>:N:` (rustfmt 1.8+, and what this machine's rustfmt prints). Only the first was
      # stripped, so from the day the header changed every unformatted file, this slice's own included, was filed as
      # `theirs` and FMT could not fail a slice (`2026-10-04`: a slice listing its own two unformatted test files under
      # "pre-existing, NOT this slice", while `mine` stayed empty). Both shapes are stripped now.
      rel="${rel% at line*}"
      rel="${rel%:[0-9]*:}"
      case "$rel" in "$root"/*) rel="${rel#"$root"/}" ;; esac
      if printf '%s\n' "$changed_files" | grep -Fxq "$rel"; then
        mine="${mine}${rel}"$'\n'
      else
        theirs="${theirs}${rel}"$'\n'
      fi
    done <<< "$unformatted"
    if [ -n "$mine" ]; then
      fmt="FAIL ($((SECONDS - started))s)"
      failed="FMT — rustfmt is not clean in a file this slice changed"
      printf '%s\n' "$mine" >&2
      echo "  fix: cargo fmt --all — never by widening the deferral list" >&2
    elif printf '%s\n' "$fmt_raw" | grep -q '^error'; then
      fmt="UNKNOWN ($((SECONDS - started))s)"
      failed="FMT — rustfmt itself failed (that is not a formatting diff)"
      printf '%s\n' "$fmt_raw" | grep '^error' | head -5 >&2
    elif [ -n "$theirs" ]; then
      fmt="PASS ($((SECONDS - started))s) — pre-existing, NOT this slice: $(printf '%s' "$theirs" | tr '\n' ' ')"
    else
      fmt="PASS ($((SECONDS - started))s)"
    fi
  fi
fi

# --- T1 — the sections you touched ------------------------------------------------------------------------------
if [ -z "$failed" ]; then
  echo "slice-check: T1 — $mode"
  started=$SECONDS
  if ! cargo build --manifest-path Cargo.toml -p cli --quiet >"$log" 2>&1; then
    t1="NOT RUN — the gate binary (cli) does not compile"
    failed="T1 — cli does not compile, so no section could be run"
    cat "$log" >&2
  else
    runner=(cargo run --manifest-path Cargo.toml -p cli --quiet -- forge test-section --changed)
    if [ -n "$since" ]; then
      runner+=(--since "$since")
    fi
    # tee, not capture: a section takes minutes, and a silent gate reads as a hung one.
    if "${runner[@]}" 2>&1 | tee "$log"; then
      t1="PASS ($((SECONDS - started))s)"
    else
      t1="FAIL ($((SECONDS - started))s)"
      failed="T1 — a test in a section you touched failed (the output above is whole, not a summary)"
    fi
    sections="$(grep -m1 '^sections to run:' "$log" | sed -e 's/^sections to run:[[:space:]]*//' || true)"
    crates_line="$(grep -m1 '^running Rust crates for sections:' "$log" || true)"
  fi
fi

# --- T2 — the whole harness, only when asked for -----------------------------------------------------------------
if [ "$full" = "1" ]; then
  echo "slice-check: T2 — the whole harness (1062 tests; CI, the nightly run and releases own this)"
  started=$SECONDS
  if command -v cargo-nextest >/dev/null 2>&1; then
    if cargo nextest run --workspace --profile ci 2>&1 | tee "$log"; then
      t2="PASS ($((SECONDS - started))s)"
    else
      t2="FAIL ($((SECONDS - started))s)"
      [ -n "$failed" ] || failed="T2 — the full suite is red"
    fi
  else
    t2name="cargo test --workspace (cargo-nextest is not installed)"
    if cargo test --manifest-path Cargo.toml --workspace 2>&1 | tee "$log"; then
      t2="PASS ($((SECONDS - started))s)"
    else
      t2="FAIL ($((SECONDS - started))s)"
      [ -n "$failed" ] || failed="T2 — the full suite is red"
    fi
  fi
fi

# --- the receipt -------------------------------------------------------------------------------------------------
if [ -z "$failed" ]; then
  result="the slice may be handed over (T0 + FMT + T1 green; T2 belongs to CI)"
else
  result="DO NOT HAND OVER — $failed"
fi

receipt_text="$(printf '%s\n' \
  "--------------------------------------------- slice-check receipt" \
  "date          $(date '+%Y-%m-%d %H:%M')" \
  "tree          $(git branch --show-current 2>/dev/null || echo detached) @ $(git rev-parse --short HEAD)" \
  "under test    $mode" \
  "changed       $changed_count file(s)" \
  "sections      ${sections:-none (nothing this taxonomy runs)}" \
  "crates        ${crates_line#running Rust crates for sections: }" \
  "T0 compile    $t0" \
  "FMT rustfmt   $fmt" \
  "T1 sections   $t1" \
  "T2 full suite $t2  [$t2name]" \
  "RESULT        $result")"

printf '\n%s\n' "$receipt_text"

if [ -n "$receipt" ]; then
  mkdir -p "$(dirname "$receipt")"
  printf '%s\n' "$receipt_text" >"$receipt"
  echo "slice-check: receipt written to $receipt"
fi

if [ -z "$failed" ]; then
  exit 0
fi
exit 1
