#!/bin/sh
# Generates the Xcode project and runs the iPhone app's UI tests on a simulator.
#   scripts/ios-test.sh                      # every UI test
#   scripts/ios-test.sh shots                # screenshots (light and dark) into docs/images/ios/
#   scripts/ios-test.sh sync                 # pairing with a desktop engine through a local relay
#   SIM="iPhone 17" scripts/ios-test.sh      # another simulator (name or UDID)
# Run scripts/build-ios-core.sh first (and after any Rust change).
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root/apps/bana-ios"
python3 "$root/scripts/gen-ios-tokens.py" --check
xcodegen generate --quiet
sim="${SIM:-iPhone 17 Pro}"
case "$sim" in
  *-*-*-*-*) dest="platform=iOS Simulator,id=$sim"; udid="$sim" ;;
  *) dest="platform=iOS Simulator,name=$sim,OS=latest"
     udid=$(xcrun simctl list devices available | grep "    $sim (" | head -1 | sed -E 's/.*\(([0-9A-F-]{36})\).*/\1/') ;;
esac
run() {
  nice xcodebuild -project Bana.xcodeproj -scheme Bana -destination "$dest" -derivedDataPath build -jobs 4 test "$@" 2>&1 \
    | grep -E "error:|Test Case .*(passed|failed)|Executed|BUILD FAILED" | sort -u
}
if [ "${1:-}" = shots ]; then
  mkdir -p "$root/docs/images/ios"
  xcrun simctl boot "$udid" 2>/dev/null || true
  for theme in light dark; do
    xcrun simctl ui "$udid" appearance "$theme"
    TEST_RUNNER_SHOTS_DIR="$root/docs/images/ios" TEST_RUNNER_SHOTS_THEME="$theme" run -only-testing:BanaUITests/ScreenshotTests/testScreens
  done
  xcrun simctl ui "$udid" appearance light
elif [ "${1:-}" = sync ]; then
  # A relay on local workerd (`wrangler dev`, nothing deployed) and a desktop engine with sync set
  # up; the test pastes a fresh pairing code into the phone. Simulators share this Mac's network.
  tmp=$(mktemp -d)
  trap 'kill $relay $desk 2>/dev/null; rm -rf "$tmp"' EXIT
  (cd "$root" && nice cargo build -q -p bana-cli)
  bin="${CARGO_TARGET_DIR:-$root/target}/debug/bana"
  rport=8795; dport=8796
  (cd "$root/cloudflare/sync" && [ -d node_modules ] || npm ci --no-audit --no-fund >/dev/null)
  (cd "$root/cloudflare/sync" && exec npx wrangler dev --port $rport --inspector-port 9295 --ip 127.0.0.1 --persist-to "$tmp/relay" >"$tmp/relay.log" 2>&1) &
  relay=$!
  BANA_HOME="$tmp/desk" "$bin" --file "$tmp/desk/graph.bana" serve --sample --today 2026-10-10 --port $dport --web "$root/web/dist" >"$tmp/desk.log" 2>&1 &
  desk=$!
  for _ in $(seq 1 300); do curl -sf "http://127.0.0.1:$rport/" >/dev/null && curl -sf "http://127.0.0.1:$dport/" >/dev/null && break; sleep 0.2; done
  token=$(cat "$tmp/desk/token")
  rpc() { curl -sf -X POST "http://127.0.0.1:$dport/rpc" -H "Authorization: Bearer $token" -d "$1"; }
  rpc "{\"method\":\"sync.setup\",\"params\":{\"relay\":\"http://127.0.0.1:$rport\",\"name\":\"Desk\"}}" >/dev/null
  for _ in $(seq 1 100); do rpc '{"method":"sync.status"}' | grep -q '"state":"synced"' && break; sleep 0.2; done
  code=$(rpc '{"method":"sync.invite"}' | python3 -c 'import sys, json; print(json.load(sys.stdin)["code"])')
  TEST_RUNNER_SYNC_CODE="$code" TEST_RUNNER_SYNC_DESK="http://127.0.0.1:$dport/" TEST_RUNNER_SYNC_TOKEN="$token" \
    run -only-testing:BanaUITests/SyncUITests
else
  run -skip-testing:BanaUITests/ScreenshotTests -skip-testing:BanaUITests/SyncUITests
fi
