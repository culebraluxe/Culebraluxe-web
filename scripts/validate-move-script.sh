#!/usr/bin/env bash
# Does the move script's per-file fix section still apply to the tree a lane worktree has?
#
# The script's `fix <file> <expression>` lines are read back out of the script and applied to the ORIGINAL file from HEAD
# (a lane has HEAD's layout, because the script is what moves it). A line whose expression no longer matches is a lane
# that silently keeps a broken path, so "no change" is a failure here, not a no-op.
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

script="scripts/restructure-domain-layout.sh"
# The commit the script is written against: the tree a lane worktree has when it runs it. HEAD is the wrong default
# now that the layout has landed — HEAD has no `rust/` left to read — so the base is named, and a lane that sits on a
# different commit passes its own: MOVE_BASE=<that commit> bash scripts/validate-move-script.sh
base="${MOVE_BASE:-pre-restructure-85bd9108}"
scratch=$(mktemp -d)
pass=0
fail=0

# The paths a file had BEFORE the move, so its original can be read out of the base commit. A file can have more than
# one: `tests/tests/*.rs` came from `rust/test-harness/tests/` (most of the suite) or from a crate's own `tests/`
# directory (`web/tests/`, `db/tests/`, `forge/tests/`).
old_paths() {
  case "$1" in
    tests/src/*) echo "rust/test-harness/src/${1#tests/src/}" ;;
    tests/tests/*)
      echo "rust/test-harness/tests/${1#tests/tests/}"
      echo "rust/server/tests/${1#tests/tests/}"
      echo "rust/forge/tests/${1#tests/tests/}"
      echo "rust/core/db/tests/${1#tests/tests/}"
      ;;
    tests/*) echo "rust/test-harness/${1#tests/}" ;;
    devops/Dockerfile.build) echo "deploy/Dockerfile.build" ;;
    devops/Dockerfile.runtime) echo "deploy/Dockerfile.runtime" ;;
    devops/rust-api/*) echo "deploy/rust-api/${1#devops/rust-api/}" ;;
    devops/Dockerfile.vercel) echo "rust/Dockerfile.vercel" ;;
    devops/Dockerfile) echo "rust/Dockerfile" ;;
    docs/rust/*) echo "rust/${1#docs/rust/}" ;;
    web/ui/*) echo "rust/ui/${1#web/ui/}" ;;
    web/auth/*) echo "rust/core/auth/${1#web/auth/}" ;;
    web/*) echo "rust/server/${1#web/}" ;;
    middle/model/*) echo "rust/core/domain/${1#middle/model/}" ;;
    middle/services/*) echo "rust/core/service/${1#middle/services/}" ;;
    middle/workflow/*) echo "rust/core/workflow/${1#middle/workflow/}" ;;
    middle/apis/*) echo "rust/integrations/${1#middle/apis/}" ;;
    db/*) echo "rust/core/db/${1#db/}" ;;
    cli/*) echo "rust/cli/${1#cli/}" ;;
    forge/*) echo "rust/forge/${1#forge/}" ;;
    *) echo "$1" ;;
  esac
}

while IFS= read -r line; do
  file=$(printf '%s' "$line" | sed -E "s/^fix ([^ ]+) '.*$/\1/")
  expression=$(printf '%s' "$line" | sed -E "s/^fix [^ ]+ '(.*)'$/\1/")
  [ -n "$file" ] && [ "$file" != "$line" ] || continue

  # The state of this file as the rules have transformed it so far — or, for a file no rule has touched yet, the
  # original from the base commit. A rule that follows another rule on the same file has to see the first one's
  # output, because that is what the script does when it runs.
  state="$scratch/state.$(old_paths "$file" | head -1 | tr '/' '_')"
  work="$scratch/file"
  if [ -f "$state" ]; then
    cp "$state" "$work"
  else
    found=0
    for candidate in $(old_paths "$file"); do
      if git show "$base:$candidate" > "$work" 2>/dev/null; then
        found=1
        break
      fi
    done
    if [ "$found" != "1" ]; then
      printf 'MISSING-ORIG  %s (tried %s)\n' "$file" "$(old_paths "$file" | tr '\n' ' ')"; fail=$((fail + 1)); continue
    fi
  fi

  before=$(shasum "$work" | cut -d' ' -f1)
  if ! EXPRESSION="$expression" perl -0pi -e '$e = $ENV{EXPRESSION}; eval "\$_ =~ $e"; die "$@" if $@;' "$work" 2>"$scratch/err"; then
    printf 'PERL-ERROR    %s: %s\n' "$file" "$(tr -d '\n' < "$scratch/err")"; fail=$((fail + 1)); continue
  fi
  after=$(shasum "$work" | cut -d' ' -f1)

  if [ "$before" = "$after" ]; then
    printf 'NO-CHANGE     %s (%s)\n' "$file" "$expression"; fail=$((fail + 1))
  else
    printf 'applies       %s\n' "$file"; pass=$((pass + 1))
  fi
  cp "$work" "$state"
done < <(grep -E "^fix " "$script")

printf '\n%s fix line(s) still apply, %s do not\n' "$pass" "$fail"
rm -rf "$scratch"
[ "$fail" -eq 0 ]
