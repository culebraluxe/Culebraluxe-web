#!/bin/bash
# Pilot: run ONE Projects-workspace story 1-by-1 (captain's rule), in DEV.
cd /Users/lisapenfieldicloud.com/Documents/Culebraluxe-web || exit 1
nohup node --env-file=.env.local --import tsx scripts/forge-engine-worker.ts \
  --story PROJECTS-WORKSPACE-01 --work-type FEATURE \
  > /tmp/pw-01.log 2>&1 &
echo "pid=$!"
