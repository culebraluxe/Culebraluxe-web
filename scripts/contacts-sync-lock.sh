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

contacts_lock_is_stale() {
  # No pid file, or a dead PID, or an over-age lock -> stale (safe to reclaim).
  if ! contacts_lock_owner_alive; then
    return 0
  fi
  local age=0
  if [ -f "$CONTACTS_SYNC_LOCK/pid" ]; then
    age=$(( $(date +%s) - $(stat -f '%m' "$CONTACTS_SYNC_LOCK/pid" 2>/dev/null || printf '%s' "$(date +%s)") ))
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
