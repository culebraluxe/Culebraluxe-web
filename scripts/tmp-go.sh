#!/bin/bash
cd /Users/lisapenfieldicloud.com/Documents/Culebraluxe-web || exit 1
story="$1"
# Persistent log dir: /tmp is periodically swept on macOS and took the run logs with
# it (the DB is the source of truth, but the chain log is what makes a run debuggable).
logdir="$HOME/Library/Logs/CulebraLuxe/forge"
mkdir -p "$logdir"
nohup node --env-file=.env.local --import tsx scripts/forge-engine-worker.ts --story "$story" --work-type FEATURE > "$logdir/$story.log" 2>&1 &
echo "pid=$! story=$story log=$logdir/$story.log"
