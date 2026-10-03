#!/usr/bin/env bash
# CulebraLuxe — unload and remove the WIP snapshot LaunchAgent.
#
# This removes the schedule only. The snapshots it already took are ordinary
# local objects under refs/wip/* and are left alone on purpose: they may be the
# only copy of somebody's half-finished work.
set -euo pipefail

HOME_DIR="${HOME}"
LABEL="com.culebraluxe.wip-snapshot"
DEST="$HOME_DIR/Library/LaunchAgents/$LABEL.plist"

launchctl bootout "gui/$(id -u)/$LABEL" 2>/dev/null || true
rm -f "$DEST"

echo "wip:uninstall: $LABEL unloaded and $DEST removed"
echo "  existing snapshots were kept: git for-each-ref refs/wip"
