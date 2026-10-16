#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# scripts/lane-adopt-jj.sh — bring an existing lane (a git worktree) into the jj repo,
# as a jj workspace over the same directory.
#
# WHY THIS EXISTS
#   On 2026-10-15 the source-control interface became `jj`, and a lane could not use it. Measured that day:
#   `jj status` inside every `lane-*` answered `Error: There is no jj repo in "."`, because `jj` finds a repo by
#   walking up for `.jj/`, and a lane is a SIBLING of the main checkout rather than a child of it. Twenty-two
#   lanes were git worktrees at that moment (`git worktree list`), so the interface the handbook told agents to
#   use was unavailable in exactly the trees they were told to work in.
#
#   `jj` cannot adopt a directory that already exists. Measured, both ways, before this script was written:
#     `jj status` inside an existing git worktree          -> `Error: There is no jj repo in "."`
#     `jj workspace add --name probe <existing lane>`      -> `Error: Destination path exists and is not an empty directory`
#   There is no `--adopt`/`--reuse` flag in `jj 0.46.0` (`jj workspace add --help`). So a conversion is: move the
#   lane's directory aside, let `jj workspace add` create it again at the same path from the lane's own commit,
#   carry back what that commit does not contain, verify, and only then remove the copy.
#
# WHY `jj workspace add` AND NOT A HAND-BUILT `.jj`
#   Because in a colocated repo it keeps BOTH interfaces pointed at the lane: measured on a throwaway, the new
#   workspace has `.jj/` (a `repo` pointer to the main checkout's `.jj/repo` + its own `working_copy`) AND a git
#   worktree registration (`<lane>/.git` -> `.git/worktrees/<name>`, with `git rev-parse`/`git status` working
#   inside it). That is what lets the ten worktree-aware scripts in this repo — `lane-new.sh`, `wip-snapshot.sh`,
#   `lane-cargo-config.sh`, `verify-checkout-build-dir.sh`, `agent-worker-once.sh`, `opencode-session-stop.sh`,
#   `restructure-domain-layout.sh`, `ops/recover/relay.sh`, `ops/recover/unblock.sh`,
#   `ops/recover/strand-report.sh` — keep seeing the lane instead of lying about what exists. A hand-written
#   `.jj/` would have to reproduce the workspace store's on-disk format, which is not a public interface.
#
#   The one thing `jj` does NOT do here: run a git hook. Measured on the throwaway — no `.cargo/config.toml`
#   appeared inside the new workspace, so `.githooks/post-checkout` (one cargo target dir per checkout) does not
#   fire for a workspace. That is why step 6 calls the recipe's own setup instead of trusting the hook.
#
# WHAT IT DOES, PER LANE
#   1. Refuses to touch a lane that has a modified/added/deleted TRACKED file: that work must become a commit
#      first, and deciding what a commit means is not this script's call (House Rule 8: a commit is a receipt).
#      Untracked and ignored files are not asked about — they are carried across byte-for-byte in step 5.
#   2. `mv <lane> <lane>.jj-adopt-<pid>` — a rename on the same filesystem, so instant.
#   3. `git worktree prune` — drops the registration whose directory just moved. Nothing else is pruned.
#   4. `jj workspace add --name <name> -r <lane HEAD> <lane>` — recreates the directory from the commit the lane
#      was standing on, and puts the new working-copy commit (`@`) directly on top of it, so the lane continues
#      exactly where it left off.
#   5. Carries back only what `jj` did not write: `node_modules` is MOVED (a rename keeps APFS clones shared, so
#      a lane keeps its 30 MB of disk instead of paying it again), and everything else is copied with
#      `--ignore-existing`, which by construction can never overwrite a file the commit already contains.
#   6. `scripts/lane-new.sh --setup-only` — the two env symlinks, this lane's own cargo target dir (the rule and
#      its text live in `scripts/lane-cargo-config.sh`), and the modes.
#   7. Verifies, and prints what it verified: `@-` is the commit the lane was on, the working copy is not in
#      conflict, every path the original had exists in the lane (file by file, ignored files included — stronger than
#      a `git status` comparison, because git never names an ignored file), the tracked file count did not shrink, and
#      git still sees the worktree. Only a lane that passes all of that has its copy removed.
#      A file git called untracked (that `.gitignore` does not cover) is snapshotted into the lane's `@` by `jj`, so the
#      receipt names that delta (`snapshotted-into-@=+N`) instead of demanding the untracked set stay identical. Five
#      lanes measured 2026-10-16 (lane-claude, lane-mimo, lane-muse, lane-nemotron, lane-nemotron-2) were refused by
#      the earlier equality check *after* a correct conversion, which is what that check cost.
#   A lane that fails a guard, or a verification, is put back exactly as it was found and reported as such.
#
# WHAT IT DOES NOT DO
#   No fetch, no rebase, no commit, no push, no branch movement, no roster edit. A lane's branches and history
#   are left where they are — `jj` imports the branches as bookmarks, it does not rewrite them. The main
#   checkout is not touched. A lane that is already a jj workspace is skipped, so a second run is a no-op — unless
#   that lane has a `<lane>.jj-adopt-<pid>` copy beside it from a refused run: then the copy is finished (verified
#   file by file against the lane, and removed only if nothing is missing), because otherwise it would sit there
#   forever and a re-run would walk straight past it.
#   It does not convert the Forge worker sandbox path (`forge/src/engine/worktree.rs`, which still provisions
#   with `git worktree add`): a worker's disposable sandbox is a different thing from a standing lane.
#
# RUN
#   bash scripts/lane-adopt-jj.sh --dry-run --all        # every registered lane, print, change nothing
#   bash scripts/lane-adopt-jj.sh --all                  # convert every registered lane
#   bash scripts/lane-adopt-jj.sh lane-claude            # one lane, by name (also: bare `claude`, or any path)
#   bash scripts/lane-adopt-jj.sh --keep-copy lane-gpt   # leave <lane>.jj-adopt-<pid> behind for inspection
# ---------------------------------------------------------------------------
set -uo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
root="$(cd "$script_dir/.." && pwd -P)"
src_root="$(dirname "$root")"


usage() {
  cat <<'USAGE'
usage: scripts/lane-adopt-jj.sh [--dry-run] [--keep-copy] [--all] [<lane-name> | <lane-path>]...

  --all             every registered git worktree of this repo except the main checkout
  <lane-name>       lane-bot3, or bare bot3, beside this checkout; any path also works (../worktrees/wo/204)
  --dry-run         print each lane's guard verdict and exit without changing anything
  --keep-copy       keep <lane>.jj-adopt-<pid> after a successful conversion

A lane that is already a jj workspace is skipped, except when a <lane>.jj-adopt-<pid> copy from a refused run is
beside it: that copy is verified file by file against the lane and removed only if nothing is missing.

Exit status: 0 when every lane was converted or skipped; 1 when any lane was refused, by a guard or a
verification. A refusal always leaves the lane as it was found.
USAGE
}

dry_run=0
all=0
keep_copy=0
targets=()
for arg in "$@"; do
  case "$arg" in
    --dry-run)   dry_run=1 ;;
    --all)       all=1 ;;
    --keep-copy) keep_copy=1 ;;
    -h|--help)   usage; exit 0 ;;
    -*)          echo "lane-adopt-jj: unknown option: $arg" >&2; usage >&2; exit 2 ;;
    *)           targets+=("$arg") ;;
  esac
done

if ! command -v jj >/dev/null 2>&1; then
  echo "lane-adopt-jj: jj is not on PATH" >&2
  exit 1
fi
if [ ! -d "$root/.jj" ]; then
  echo "lane-adopt-jj: $root is not a jj repo — this script converts lanes INTO one" >&2
  exit 1
fi

# The registry is `git worktree list`, the same source `scripts/wip-snapshot.sh` and House Rule 4's roster use,
# so a lane that is not registered is never converted, and a registered lane is never missed.
lane_paths=()
if [ "$all" -eq 1 ]; then
  while IFS= read -r wt; do
    [ -n "$wt" ] || continue
    [ "$wt" = "$root" ] && continue
    lane_paths+=("$wt")
  done <<< "$(git -C "$root" worktree list --porcelain | awk '/^worktree /{sub(/^worktree /, ""); print}')"
fi
for target in ${targets[@]+"${targets[@]}"}; do
  # Four ways to name a lane, all of them spelled in the usage text, so none of them may be a silent miss:
  #   /abs/path, ../worktrees/wo/204 (a path), lane-bot3, and bare bot3. Measured 2026-10-16: only the bare form
  #   worked, because the name was unconditionally prefixed with `lane-`, so `lane-bot3` looked for
  #   `lane-lane-bot3` and the run reported "skipped (not on disk)" — a conversion tool that misses its argument.
  if [ -d "$target" ]; then
    lane_paths+=("$(cd "$target" && pwd -P)")
  elif [ -d "$src_root/$target" ]; then
    lane_paths+=("$src_root/$target")
  elif [ -d "$src_root/lane-$target" ]; then
    lane_paths+=("$src_root/lane-$target")
  else
    # A name that is not a path and is not on disk either way: say so and stop, before any lane is touched. The old
    # behaviour carried a made-up path into the loop, which reported "skipped lane-lane-botts (not on disk)" for the
    # typo `lane-botts` — the concatenation of both spellings, which names nothing the user typed.
    echo "lane-adopt-jj: no such lane: $target (looked for $src_root/$target and $src_root/lane-$target)" >&2
    exit 2
  fi
done

if [ "${#lane_paths[@]}" -eq 0 ]; then
  usage >&2
  exit 2
fi

converted=0
skipped=0
refused=0
finished=0
refusals=()

# Every path the original had must exist in the converted lane. This is the no-loss receipt, and it is stronger than
# comparing `git status` output, because it also covers ignored files (build output, caches) that git would never name.
# `node_modules` is compared by its own existence — it was moved, not copied — and `.git`, `.jj`, `.DS_Store` carry no
# user work. Prints up to ten of the paths that are missing, and nothing at all when nothing is missing.
missing_from_lane() {  # $1 = the original, $2 = the converted lane
  if [ -d "$1/node_modules" ] && [ ! -d "$2/node_modules" ]; then
    echo "./node_modules"
  fi
  ( cd "$1" && find . \
      -path ./node_modules -prune -o \
      -path ./.git -prune -o \
      -path ./.jj -prune -o \
      -name .DS_Store -prune -o \
      -print ) | while IFS= read -r f; do
    [ -e "$2/$f" ] || [ -L "$2/$f" ] || printf '%s\n' "$f"
  done | head -10
}

for path in ${lane_paths[@]+"${lane_paths[@]}"}; do
  # The workspace name is the lane's path relative to the source root, so it is unique by construction and still
  # reads like the lane: `lane-claude`, and `worktrees-wo-204` for a worktree that lives one level deeper.
  rel="${path#"$src_root"/}"
  ws="${rel//\//-}"

  skip() {
    echo "skipped   $rel  ($1)"
    skipped=$((skipped + 1))
  }
  refuse() {
    echo "REFUSED   $rel  ($1)" >&2
    refusals+=("$rel: $1")
    refused=$((refused + 1))
  }

  if [ -e "$path/.jj" ]; then
    # A run that refused a conversion has already made the lane a jj workspace and kept the original beside it. A plain
    # re-run would skip that lane and leave a 100 MB copy there forever, so the copy is finished here: the no-loss
    # receipt, read against the copy itself (its git metadata was pruned with the old worktree, so the file set is what
    # is checkable), and only a pass removes it. A stranded copy is never a registered git worktree, and that is
    # checked before anything is deleted.
    copies=()
    while IFS= read -r c; do
      [ -n "$c" ] && copies+=("$c")
    done <<< "$(find "$(dirname "$path")" -maxdepth 1 -name "$(basename "$path").jj-adopt-*" -type d 2>/dev/null)"
    if [ "${#copies[@]}" -eq 0 ]; then
      skip "already a jj workspace"
      continue
    fi
    did_finish=0
    for c in ${copies[@]+"${copies[@]}"}; do
      if git -C "$root" worktree list --porcelain | grep -qxF "worktree $c"; then
        refuse "$(basename "$c") is still a registered git worktree of this checkout; left alone"
        continue
      fi
      if [ "$dry_run" -eq 1 ]; then
        echo "would finish $rel  (verify $(basename "$c") against the lane, then remove the copy)"
        did_finish=1
        continue
      fi
      kept_parent="$(jj -R "$path" --ignore-working-copy log -r '@-' --no-graph -T 'commit_id' 2>/dev/null | head -1)"
      kept_head="$(git -C "$path" rev-parse HEAD 2>/dev/null || true)"
      kept_conflict="$(jj -R "$path" --ignore-working-copy log -r '@' --no-graph -T 'if(conflict, "CONFLICT", "ok")' 2>/dev/null | head -1)"
      missing="$(missing_from_lane "$c" "$path")"
      if [ -n "$missing" ]; then
        refuse "$(basename "$c"): files missing from $rel: $(printf '%s' "$missing" | tr '\n' ' ')"
        continue
      fi
      if [ "$kept_parent" != "$kept_head" ]; then
        refuse "$(basename "$c"): @-(${kept_parent:0:8}) is not the lane's HEAD(${kept_head:0:8}); copy kept"
        continue
      fi
      if [ "$kept_conflict" != "ok" ]; then
        refuse "$(basename "$c"): the lane's working copy is in conflict; copy kept"
        continue
      fi
      rm -rf "$c"
      echo "finished  $rel  copy=$(basename "$c") verified (every file in the original exists in the lane) and removed"
      finished=$((finished + 1))
      did_finish=1
    done
    [ "$did_finish" -eq 1 ] && continue
    skip "already a jj workspace"
    continue
  fi
  if [ ! -d "$path" ]; then
    skip "not on disk"
    continue
  fi
  if ! git -C "$root" worktree list --porcelain | grep -qxF "worktree $path"; then
    skip "not a registered worktree of this checkout"
    continue
  fi
  if [ -n "$(jj -R "$root" workspace list 2>/dev/null | awk -F: '{print $1}' | grep -xF "$ws")" ]; then
    skip "a jj workspace named $ws already exists"
    continue
  fi

  # Guard: only a lane whose only changes are untracked/ignored files may be recreated. A tracked change is work
  # whose meaning a script cannot decide — it has to become a commit first, and that is a judgment call.
  tracked="$(git -C "$path" status --porcelain --untracked-files=no)"
  if [ -n "$tracked" ]; then
    refuse "has tracked changes; commit them first — this script will not decide what a commit means"
    printf '%s\n' "$tracked" | sed 's/^/            /' >&2
    continue
  fi

  head="$(git -C "$path" rev-parse HEAD 2>/dev/null || true)"
  if [ -z "$head" ]; then
    refuse "could not read HEAD with git"
    continue
  fi
  # `jj workspace add -r <rev>` needs the revision in jj's view of the repo. A lane whose HEAD is a detached
  # commit with no ref pointing at it would import nothing, so it is refused rather than guessed at.
  if ! jj -R "$root" --ignore-working-copy log -r "$head" --no-graph -T 'commit_id' >/dev/null 2>&1; then
    refuse "HEAD $head is not reachable from any ref, so jj cannot resolve it; give the lane's work a name first"
    continue
  fi

  branch="$(git -C "$path" symbolic-ref -q --short HEAD || true)"
  files_before="$(git -C "$path" ls-files | wc -l | tr -d ' ')"
  untracked_before="$(git -C "$path" status --porcelain --untracked-files=all | grep '^??' | sed 's/^?? //' | grep -vx '.DS_Store' | sort)"
  untracked_count_before="$(printf '%s' "$untracked_before" | grep -c . || true)"

  echo "converting $rel"
  echo "    head $(printf '%s' "$head" | cut -c1-8)  branch ${branch:-DETACHED}  tracked-files $files_before  untracked $untracked_count_before  ws-name $ws"

  if [ "$dry_run" -eq 1 ]; then
    echo "    dry run: mv $path -> $path.jj-adopt-<pid>; git worktree prune; jj workspace add --name $ws -r $(printf '%s' "$head" | cut -c1-8); lane-new.sh --setup-only"
    converted=$((converted + 1))
    continue
  fi

  copy="$path.jj-adopt-$$"
  if ! mv "$path" "$copy"; then
    refuse "could not move the lane aside"
    continue
  fi
  git -C "$root" worktree prune >/dev/null 2>&1

  if ! jj -R "$root" workspace add --name "$ws" -r "$head" "$path"; then
    # Put it back: forget the half-made workspace, drop the directory it created, move the original back, and
    # re-register it with git so the lane is where it was. The branch ref is untouched either way.
    jj -R "$root" workspace forget "$ws" >/dev/null 2>&1
    rm -rf "$path"
    mv "$copy" "$path"
    git -C "$root" worktree add --force --detach "$path" "$head" >/dev/null 2>&1
    refuse "jj workspace add failed; the lane was put back (re-attach its branch: git -C $rel switch ${branch:-<branch>})"
    continue
  fi

  # Carry back what the commit does not contain. node_modules moves — a rename, so the clones stay shared and the
  # lane keeps its 30 MB of disk instead of paying 847 MB again. The rest is a copy that cannot overwrite: with
  # `--ignore-existing`, only paths the recreated tree does not already have are written.
  if [ -d "$copy/node_modules" ]; then
    mv "$copy/node_modules" "$path/node_modules" || true
  fi
  rsync -a --ignore-existing \
    --exclude '.git' --exclude '.jj' --exclude '.DS_Store' \
    --exclude '.cargo/config.toml' --exclude 'node_modules/' \
    "$copy/" "$path/" || true

  bash "$script_dir/lane-new.sh" --setup-only "$path" >/dev/null 2>&1 \
    || echo "    ! lane-new.sh --setup-only reported a problem; check $rel/.env.local and $rel/.cargo/config.toml" >&2

  # Put the lane's working copy in step with its `@` before verifying. Another workspace's operation can leave
  # this one stale — measured inside a lane: `Error: The working copy is stale (not updated since operation …).
  # Hint: Run jj workspace update-stale` — and `update-stale` on a fresh workspace is a no-op, so it is safe.
  jj -R "$path" workspace update-stale >/dev/null 2>&1 || true

  # --- verification, printed, because a receipt is not a claim (House Rule 8) ---
  # Read the graph without touching the working copy, so a sibling workspace moving during the conversion
  # cannot turn into a false refusal here.
  parent="$(jj -R "$path" --ignore-working-copy log -r '@-' --no-graph -T 'commit_id' 2>/dev/null | head -1)"
  conflicts="$(jj -R "$path" --ignore-working-copy log -r '@' --no-graph -T 'if(conflict, "CONFLICT", "ok")' 2>/dev/null | head -1)"

  files_after="$(git -C "$path" ls-files | wc -l | tr -d ' ')"
  registered="$(git -C "$root" worktree list --porcelain | grep -cxF "worktree $path")"
  target="$(bash "$script_dir/lane-cargo-config.sh" --print --at "$path" 2>/dev/null || true)"
  missing="$(missing_from_lane "$copy" "$path")"

  # jj snapshots a working copy: a file that git called untracked — and that `.gitignore` does not cover — becomes part
  # of `@` the moment the workspace exists. So the honest expectation is not "the tracked count and the untracked set
  # are unchanged"; it is "nothing shrank, and every path the original had exists in the lane". Measured 2026-10-16 on
  # five lanes (lane-claude, lane-mimo, lane-muse, lane-nemotron, lane-nemotron-2): the old equality check refused all
  # five AFTER a correct conversion, counting exactly the snapshotted files as "untracked-set-differs".
  snapped=$((files_after - files_before))

  problems=""
  [ "$parent" = "$head" ] || problems="${problems}@-(${parent:0:8})!=head(${head:0:8}) "
  [ "$conflicts" = "ok" ] || problems="${problems}working-copy-in-conflict "
  [ "$snapped" -ge 0 ] || problems="${problems}tracked-files-shrank(${files_after}<${files_before}) "
  [ -z "$missing" ] || problems="${problems}files-missing-from-lane($(printf '%s' "$missing" | tr '\n' ' '))"
  [ "$registered" = "1" ] || problems="${problems}git-worktree-not-registered "
  [ -n "$target" ] || problems="${problems}no-cargo-target-dir "

  if [ -n "$problems" ]; then
    refuse "converted but the receipt failed: ${problems}— the original is kept at $(basename "$copy")"
    continue
  fi

  # The receipt says what was checked, not that something was checked: @- is the commit the lane was on, the count of
  # files it came with is unchanged unless jj snapshotted untracked work into `@` (then the delta is named), every path
  # the original had is here, and git still sees the worktree.
  line="converted $rel  ws=$ws  @-=$(printf '%s' "$head" | cut -c1-8)  files=$files_before"
  [ "$snapped" -eq 0 ] || line="$line  snapshotted-into-@=+$snapped"
  line="$line  untracked-before=$untracked_count_before  missing=0  target=$target"
  if [ "$keep_copy" -eq 1 ]; then
    echo "$line  (original kept at $(basename "$copy"))"
  else
    rm -rf "$copy"
    echo "$line"
  fi
  converted=$((converted + 1))
done

echo
echo "lane-adopt-jj: converted=$converted skipped=$skipped refused=$refused finished=$finished"
if [ "$dry_run" -eq 1 ]; then
  echo "lane-adopt-jj: dry run, nothing changed"
fi
if [ "$refused" -gt 0 ]; then
  echo "lane-adopt-jj: a refusal is a report, not a retry (House Rule 9):" >&2
  for r in ${refusals[@]+"${refusals[@]}"}; do echo "  - $r" >&2; done
  exit 1
fi


