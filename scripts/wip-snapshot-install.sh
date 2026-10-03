#!/usr/bin/env bash
# CulebraLuxe — install/refresh the WIP snapshot LaunchAgent (every 5 minutes).
#
# The job is deliberately boring: it only ever writes local refs/wip/<name>
# commits, so installing it cannot block, push or rebase anything. Uninstall
# with `pnpm wip:uninstall`; the snapshot refs it leaves behind stay in the
# repository (they are local, unpushed objects — delete them with
# `git update-ref -d refs/wip/<name>` if you ever want them gone).
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
HOME_DIR="${HOME}"
LOG_DIR="${CULEBRALUXE_WIP_LOG_DIR:-/Users/Shared/dev/build/logs}"
LABEL="com.culebraluxe.wip-snapshot"
TEMPLATE="$REPO/scripts/$LABEL.plist.template"
DEST="$HOME_DIR/Library/LaunchAgents/$LABEL.plist"

if [ ! -f "$TEMPLATE" ]; then
  echo "wip:install: template missing: $TEMPLATE" >&2
  exit 1
fi
if [ ! -f "$REPO/scripts/wip-snapshot.sh" ]; then
  echo "wip:install: worker missing: $REPO/scripts/wip-snapshot.sh" >&2
  exit 1
fi

mkdir -p "$HOME_DIR/Library/LaunchAgents" "$LOG_DIR"

# PATH is the one the other CulebraLuxe agents get: the node/nvm bin directory
# first, then the system ones. git lives in /usr/bin, but a job that cannot find
# npm/node the way the others do is a job that behaves differently to fix.
PATH_VALUE="$(dirname "$(command -v node 2>/dev/null || echo /opt/homebrew/bin/node)"):/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

sed -e "s#{{REPO_ROOT}}#$REPO#g" \
    -e "s#{{HOME}}#$HOME_DIR#g" \
    -e "s#{{LOG_DIR}}#$LOG_DIR#g" \
    -e "s#{{PATH}}#$PATH_VALUE#g" \
    "$TEMPLATE" > "$DEST"

plutil -lint "$DEST" >/dev/null

# bootout first: bootstrap refuses to replace an already-loaded job, and a stale
# copy of the worker path is exactly what an upgrade is supposed to fix.
launchctl bootout "gui/$(id -u)/$LABEL" 2>/dev/null || true
launchctl bootstrap "gui/$(id -u)" "$DEST"
launchctl enable "gui/$(id -u)/$LABEL" 2>/dev/null || true

echo "wip:install: $LABEL loaded -> $DEST"
echo "  worker : $REPO/scripts/wip-snapshot.sh"
echo "  cadence: every 300s, plus once at load"
echo "  logs   : $LOG_DIR/wip-snapshot.log"
echo "  run it : pnpm wip:now"
