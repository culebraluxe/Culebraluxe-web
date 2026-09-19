#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# CulebraLuxe — Contacts sync single-flight lock (sourced library).
#
# An atomic `mkdir` lock so two Apple Contacts PROD syncs can never run at the
# same time on one Mac (scheduled/background vs. manual operator run). It is
# deliberately DISTINCT from the Apple Messages sync lock
# (/tmp/culebraluxe-apple-sync.lock) so the two agents never collide.
#
# Behavior:
#   - acquire returns 0 when this process owns the lock (creating it atomically)
#   - acquire returns 1 when ANOTHER live process owns it (operator FAILS; the
#     wrapper does NOT retry or skip)
#   - a stale lock (dead PID, or a lock older than CONTACTS_SYNC_LOCK_MAX_AGE)
#     is reclaimed conservatively
#   - release removes the lock only when this process owns it
# ---------------------------------------------------------------------------
CONTACTS_SYNC_LOCK="${CONTACTS_SYNC_LOCK:-/tmp/culebraluxe-contacts-sync.lock}"
# Conservative stale-age guard: reclaim a lock older than 4h even if the PID
# file is still readable (prevents a wedged run from blocking future syncs).
CONTACTS_SYNC_LOCK_MAX_AGE=14400

contacts_lock_pid() {
  [ -f "$CONTACTS_SYNC_LOCK/pid" ] && cat "$CONTACTS_SYNC_LOCK/pid" 2>/dev/null || true
}

contacts_lock_owner_alive() {
  local pid
  pid="$(contacts_lock_pid)"
  [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null
}

# The lock file's mtime in epoch seconds, PORTABLY.
#
# MEASURED 2026-09-18, and it broke the one guarantee this library exists for: the age check used BSD
# `stat -f '%m'` alone. On GNU/Linux (CI) `-f` means FILESYSTEM status and `%m` is not a filesystem
# directive, so the arithmetic got non-numeric output, `contacts_lock_is_stale` answered WRONGLY, and the
# lock was reclaimed while a live process held it — reproduced by shimming `stat` to GNU behaviour. Both
# spellings are tried now and the answer must be DIGITS; anything else means "now", i.e. age 0, i.e.
# conservatively NOT stale. Reclaiming a live lock is worse than waiting out a dead one.
contacts_lock_mtime() {
  local path="$1" mtime=""
  mtime="$(stat -c '%Y' "$path" 2>/dev/null)"
  case "$mtime" in '' | *[!0-9]*) mtime="$(stat -f '%m' "$path" 2>/dev/null)" ;; esac
  case "$mtime" in '' | *[!0-9]*) mtime="$(date +%s)" ;; esac
  printf '%s' "$mtime"
}

contacts_lock_is_stale() {
  # No pid file, or a dead PID, or an over-age lock -> stale (safe to reclaim).
  if ! contacts_lock_owner_alive; then
    return 0
  fi
  local age=0
  if [ -f "$CONTACTS_SYNC_LOCK/pid" ]; then
    age=$(( $(date +%s) - $(contacts_lock_mtime "$CONTACTS_SYNC_LOCK/pid") ))
  fi
  [ "$age" -gt "$CONTACTS_SYNC_LOCK_MAX_AGE" ]
}

contacts_acquire_lock() {
  if mkdir "$CONTACTS_SYNC_LOCK" 2>/dev/null; then
    echo "$$" > "$CONTACTS_SYNC_LOCK/pid"
    return 0
  fi
  if contacts_lock_is_stale; then
    rm -rf "$CONTACTS_SYNC_LOCK"
    if mkdir "$CONTACTS_SYNC_LOCK" 2>/dev/null; then
      echo "$$" > "$CONTACTS_SYNC_LOCK/pid"
      return 0
    fi
  fi
  return 1
}

contacts_release_lock() {
  rm -rf "$CONTACTS_SYNC_LOCK"
}
