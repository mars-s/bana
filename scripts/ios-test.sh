#!/bin/sh
# Generates the Xcode project and runs the iPhone app's UI tests on a simulator.
#   scripts/ios-test.sh                      # every UI test
#   scripts/ios-test.sh shots                # screenshots (light and dark) into docs/images/ios/
#   SIM="iPhone 17" scripts/ios-test.sh      # another simulator (name or UDID)
# Run scripts/build-ios-core.sh first (and after any Rust change).
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root/apps/tanacraft-ios"
python3 "$root/scripts/gen-ios-tokens.py" --check
xcodegen generate --quiet
sim="${SIM:-iPhone 17 Pro}"
case "$sim" in
  *-*-*-*-*) dest="platform=iOS Simulator,id=$sim"; udid="$sim" ;;
  *) dest="platform=iOS Simulator,name=$sim,OS=latest"
     udid=$(xcrun simctl list devices available | grep "    $sim (" | head -1 | sed -E 's/.*\(([0-9A-F-]{36})\).*/\1/') ;;
esac
run() {
  nice xcodebuild -project TanaCraft.xcodeproj -scheme TanaCraft -destination "$dest" -derivedDataPath build -jobs 4 test "$@" 2>&1 \
    | grep -E "error:|Test Case .*(passed|failed)|Executed|BUILD FAILED" | sort -u
}
if [ "${1:-}" = shots ]; then
  mkdir -p "$root/docs/images/ios"
  xcrun simctl boot "$udid" 2>/dev/null || true
  for theme in light dark; do
    xcrun simctl ui "$udid" appearance "$theme"
    TEST_RUNNER_SHOTS_DIR="$root/docs/images/ios" TEST_RUNNER_SHOTS_THEME="$theme" run -only-testing:TanaCraftUITests/ScreenshotTests/testScreens
  done
  xcrun simctl ui "$udid" appearance light
else
  run -skip-testing:TanaCraftUITests/ScreenshotTests
fi
