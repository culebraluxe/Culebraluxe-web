#!/bin/bash
cd /Users/lisapenfieldicloud.com/Documents/Culebraluxe-web || exit 1
nohup node --env-file=.env.local --import tsx scripts/forge-engine-worker.ts \
  --story "$1" --work-type FEATURE > "/tmp/forge-$1.log" 2>&1 &
echo "pid=$! story=$1 log=/tmp/forge-$1.log"
