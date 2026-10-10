#!/bin/sh
# Builds the Rust engine for the iPhone app: a static library for device (aarch64-apple-ios) and
# simulator (aarch64-apple-ios-sim), Swift bindings from UniFFI, and one XCFramework.
#
#   scripts/build-ios-core.sh            # release build (what the app ships)
#   PROFILE=debug scripts/build-ios-core.sh
#
# Output (git-ignored): apps/bana-ios/Core/BanaCore.xcframework and
# apps/bana-ios/Core/Generated/bana_ffi.swift.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target-ios}"
export IPHONEOS_DEPLOYMENT_TARGET=26.0
profile="${PROFILE:-release}"
flag=""
[ "$profile" = release ] && flag="--release"
jobs="${JOBS:-4}"

for t in aarch64-apple-ios aarch64-apple-ios-sim; do
  nice cargo build -j "$jobs" -p bana-ffi --lib $flag --target "$t"
done

out="$root/apps/bana-ios/Core"
gen="$out/Generated"
hdr="$CARGO_TARGET_DIR/ios-headers"
rm -rf "$gen" "$hdr" "$out/BanaCore.xcframework"
mkdir -p "$gen" "$hdr"

lib_sim="$CARGO_TARGET_DIR/aarch64-apple-ios-sim/$profile/libbana_ffi.a"
lib_dev="$CARGO_TARGET_DIR/aarch64-apple-ios/$profile/libbana_ffi.a"

# Bindings come from the metadata inside the library itself (UniFFI library mode).
nice cargo run -j "$jobs" -q -p bana-ffi --features bindgen --bin uniffi-bindgen -- \
  generate --library "$lib_sim" --language swift --out-dir "$hdr"
mv "$hdr/bana_ffi.swift" "$gen/"
mv "$hdr/bana_ffiFFI.modulemap" "$hdr/module.modulemap"

xcodebuild -create-xcframework \
  -library "$lib_dev" -headers "$hdr" \
  -library "$lib_sim" -headers "$hdr" \
  -output "$out/BanaCore.xcframework" >/dev/null
echo "built $out/BanaCore.xcframework and $gen/bana_ffi.swift"
