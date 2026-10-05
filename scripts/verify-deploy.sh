#!/usr/bin/env bash
# Post-deploy verification against PRODUCTION itself. Usage: verify-deploy.sh <expected-sha>
#
# WHAT IT CHECKS
#   1. the build stamp answers the deployed commit sha, and databaseTarget=prod
#   2. the three changed pictures are byte-for-byte what the tree ships
#   3. assets that are mode 600 in the tree are still served — the proof of the stage fix in deploy-prod.sh
#   4. the deploy script's own paths answer 200
#   5. every file under public/ the deploy ships answers 200 with the right bytes
#   6. the private staging root (public/upload/data, which the deploy excludes) is NOT downloadable — the
#      assertion that would have caught the 2026-10-05 exposure, and the one that keeps it fixed
#
# THE URL MAPPING IS THE WHOLE TRICK: the server's static root IS public/, so public/images/x.jpg is served at
# /images/x.jpg. Asking for /public/images/x.jpg returns the SPA fallback page as a 200, so six different images all
# "match" the homepage's md5. The first version of this script did exactly that and reported six false MISMATCHes on
# 2026-10-05 — fix the mapping, not the images.
#
# AND THE OTHER TRICK IS ENCODING: two shipped filenames contain a space, and curl refuses a URL with a raw space
# ("Malformed input to a URL function") — the second version of this script reported those two as `code 000`, which
# is the harness failing, not the server. urlenc percent-encodes every byte outside the unreserved set, byte-wise
# under LC_ALL=C, so spaces, quotes, `#`, `%` and any future non-ASCII name are asked for correctly.
#
# ONE RUN, ONE LOG FILE. Two copies of this script writing the same log interleave their output and the line count
# freezes — it happened on 2026-10-05 and it costs the honest half of the receipt. Redirect to a fresh
# `build/logs/verify-<sha>.txt` yourself, and wait for `VERIFY_DONE=` before reading it: that line is present only
# when the run ended, and its value is the exit code.
set -uo pipefail
SHA="${1:?usage: verify-deploy.sh <expected-sha>}"
SITE=https://www.culebraluxe.com
ROOT=/Users/Shared/dev/src/Culebraluxe-web
BUST="?verify=$SHA"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
fail=0

urlenc() { # $1 = a path; prints it percent-encoded, one byte at a time, leaving / and unreserved chars alone
  local LC_ALL=C s="$1" out='' c i
  for ((i = 0; i < ${#s}; i++)); do
    c="${s:i:1}"
    case "$c" in
      [a-zA-Z0-9._~/-]) out+="$c" ;; # note: `-` last, so `/` and `-` stay literal (a range `_-/` matches neither)
      *) out+="$(printf '%%%02X' "'$c")" ;;
    esac
  done
  printf '%s' "$out"
}

echo "=== 0. reference tree"
echo "  deployed sha: $SHA"
echo "  tree HEAD:    $(git -C "$ROOT" rev-parse HEAD)"

echo
echo "=== 1. the build stamp"
info="$(curl -s --max-time 30 "$SITE/api/build-info")"
echo "  $info"
case "$info" in *"$SHA"*) echo "  OK    stamp carries $SHA" ;; *) echo "  FAIL  stamp does not carry $SHA"; fail=1 ;; esac
case "$info" in *'"databaseTarget":"prod"'*) echo "  OK    databaseTarget=prod" ;; *) echo "  FAIL  databaseTarget is not prod"; fail=1 ;; esac

fetch() { # $1 = repo-relative path under public/ ; prints the status code, leaves the body in $TMP/body
  # the `/` after $SITE is load-bearing: without it the host becomes "www.culebraluxe.comimages" and curl answers
  # nothing at all (000, no body file) — which reads exactly like a mismatch if you do not label it.
  curl -s --retry 2 --retry-delay 1 --max-time 30 -o "$TMP/body" -w '%{http_code}' "$SITE/$(urlenc "${1#public/}")$BUST"
}
check() { # $1 = repo-relative path under public/
  local want got code
  want="$(md5 -q "$ROOT/$1")"
  code="$(fetch "$1")"
  got="$(md5 -q "$TMP/body")"
  if [ "$code" = "200" ] && [ "$want" = "$got" ]; then
    printf '  MATCH     %s  %s\n' "$code" "$1"
  elif [ "$code" = "000" ]; then
    printf '  TRANSPORT (no answer at all — network/harness, NOT the server)  %s\n' "$1"
    fail=1
  else
    printf '  MISMATCH %s  want=%s got=%s  %s\n' "$code" "$want" "$got" "$1"
    fail=1
  fi
}

echo
echo "=== 2. the three changed pictures, byte-for-byte"
check public/images/about/lisa-portrait.jpg
check public/images/about/lisa-work.jpg
check public/images/services/service-07-real-estate-consultation.jpg

echo
echo "=== 3. assets that are mode 600 in the tree and were NOT touched by these commits"
echo "    (the proof of the stage fix: unreadable in the image without it)"
for f in public/images/about/about-hero.jpg public/images/services/service-01-market-analysis-cma.jpg public/images/about/life-01.jpg; do
  printf '  mode=%s  %s\n' "$(stat -f '%Lp' "$ROOT/$f")" "$f"
  check "$f"
done

echo
echo "=== 4. the deploy script's own paths"
for p in / /buyers /app.css /rust-ui/ui.js /rust-ui/ui_bg.wasm "/api/rust-ui/public-page?screen=site-home" /login; do
  code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 30 "$SITE$p")"
  printf '  %s  %s\n' "$code" "$p"
  [ "$code" = "200" ] || fail=1
done

echo
echo "=== 5. full sweep: every file under public/ that the deploy ships"
big=8388608   # 8 MB: above this, asking HEAD for the length beats pulling the body on every run
total=0; bad=0
while IFS= read -r f; do
  rel="${f#public/}"
  # `upload/data` is excluded from the artifact on purpose (scripts/deploy-prod.sh); §6 asserts it is NOT
  # reachable, so demanding it here as "must be served" would contradict the fix.
  case "$rel" in upload/data/*) continue ;; esac
  total=$((total + 1))
  size="$(stat -f '%z' "$ROOT/$f")"
  if [ "$size" -gt "$big" ]; then
    code="$(curl -sI --retry 2 --retry-delay 1 --max-time 30 -o "$TMP/head" -w '%{http_code}' "$SITE/$(urlenc "$rel")$BUST")"
    served="$(sed -n 's/^[Cc]ontent-[Ll]ength: *\([0-9]*\).*/\1/p' "$TMP/head" | tr -d '\r')"
    if [ "$code" = "200" ] && [ "$served" = "$size" ]; then
      printf '  SIZE-CHECK %s  %s (%s bytes, length verified without pulling the body)\n' "$code" "$rel" "$size"
    else
      bad=$((bad + 1))
      printf '  WRONG SIZE (code %s, content-length=%s, on disk=%s)  %s\n' "$code" "${served:-none}" "$size" "$rel"
    fi
    continue
  fi
  code="$(curl -s --retry 2 --retry-delay 1 -o "$TMP/body" -w '%{http_code}' --max-time 30 "$SITE/$(urlenc "$rel")$BUST")"
  if [ "$code" != "200" ] || [ "$(md5 -q "$ROOT/$f")" != "$(md5 -q "$TMP/body")" ]; then
    bad=$((bad + 1))
    if [ "$code" = "000" ]; then
      printf '  TRANSPORT (no answer — network/harness, NOT the server)  %s\n' "$rel"
    else
      printf '  NOT SERVED AS SHIPPED (code %s)  %s\n' "$code" "$rel"
    fi
  fi
done < <(cd "$ROOT" && find public -type f ! -path 'public/rust-ui/*' ! -name '* 2' ! -name '* 2.*' | sort)
echo "  checked $total files; $bad wrong"

# Section 6 is the one that would have caught the 2026-10-05 exposure before it shipped, and it is written as a
# negative assertion on purpose: the staging root exists in the tree (the exporters write there) and must NOT
# exist on the site. §5 cannot see it — §5 only knows what the deploy ships, and this is exactly what it must
# not. A 200 whose content-length equals the file's size on disk IS the file: the SPA fallback page never has
# that length, so a HEAD settles it without pulling 64 MB through the wire.
echo
echo "=== 6. private staging root is NOT downloadable (public/upload/data, excluded from the artifact)"
staged=0; leaks=0
while IFS= read -r f; do
  [ -n "$f" ] || continue
  rel="${f#public/}"
  size="$(stat -f '%z' "$ROOT/$f")"
  code="$(curl -sI --retry 2 --retry-delay 1 --max-time 30 -o "$TMP/head" -w '%{http_code}' "$SITE/$(urlenc "$rel")$BUST")"
  len="$(sed -n 's/^[Cc]ontent-[Ll]ength: *\([0-9]*\).*/\1/p' "$TMP/head" | tr -d '\r')"
  staged=$((staged + 1))
  if [ "$code" = "200" ] && [ "$len" = "$size" ]; then
    leaks=$((leaks + 1)); fail=1
    printf '  LEAK  %s/%s is downloadable (%s bytes) — the deploy exclusion did not hold\n' "$SITE" "$rel" "$size"
  fi
done < <(cd "$ROOT" && find public/upload/data -type f 2>/dev/null | sort)
printf '  %s staging file(s) on disk, %s reachable\n' "$staged" "$leaks"

echo
if [ "$fail" = 0 ] && [ "$bad" = 0 ]; then echo "VERIFY: PASS (deploy integrity)"; else echo "VERIFY: FAIL (deploy integrity)"; fi
if [ "$leaks" = 0 ]; then echo "PRIVACY: clean — nothing staged under public/upload/data is served"; else echo "PRIVACY: $leaks leaked path(s) — see LEAK above"; fi
printf 'VERIFY_DONE=%s\n' "$fail"
exit "$fail"

