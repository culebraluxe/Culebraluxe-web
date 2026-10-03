#!/usr/bin/env bash
# Does the move script's per-file fix section still apply to the tree a lane worktree has?
#
# The script's `fix <file> <expression>` lines are read back out of the script and applied to the ORIGINAL file from HEAD
# (a lane has HEAD's layout, because the script is what moves it). A line whose expression no longer matches is a lane
# that silently keeps a broken path, so "no change" is a failure here, not a no-op.
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

script="scripts/restructure-domain-layout.sh"
scratch=$(mktemp -d)
pass=0
fail=0

# The path a file had before the move, so its original can be read out of HEAD.
old_path() {
  case "$1" in
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

  original=$(old_path "$file")
  work="$scratch/file"
  if ! git show "HEAD:$original" > "$work" 2>/dev/null; then
    # A second `fix` on the same file re-reads the file the first one just fixed, which is what the script does too.
    if [ ! -f "$scratch/last" ]; then
      printf 'MISSING-ORIG  %s (tried %s)\n' "$file" "$original"; fail=$((fail + 1)); continue
    fi
    cp "$scratch/last" "$work"
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
  cp "$work" "$scratch/last"
done < <(grep -E "^fix " "$script")

printf '\n%s fix line(s) still apply, %s do not\n' "$pass" "$fail"
rm -rf "$scratch"
[ "$fail" -eq 0 ]
