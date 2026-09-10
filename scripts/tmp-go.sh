#!/bin/bash
cd /Users/lisapenfieldicloud.com/Documents/Culebraluxe-web || exit 1
story="$1"
nohup node --env-file=.env.local --import tsx scripts/forge-engine-worker.ts --story "$story" --work-type FEATURE > "/tmp/forge-${story}.log" 2>&1 &
echo "pid=$! story=$story"
