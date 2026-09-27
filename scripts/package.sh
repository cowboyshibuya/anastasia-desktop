#!/usr/bin/env bash
set -euo pipefail

desktop_source="$(cd "$(dirname "$0")/.." && pwd)"
engine_source="${ANASTASIA_ENGINE_SOURCE:-$desktop_source/../anastasia-engine}"
engine_source="$(cd "$engine_source" && pwd)"
pinned_revision="$(sed -n 's/.*anastasia-engine", rev = "\([0-9a-f]*\)".*/\1/p' "$desktop_source/Cargo.toml" | head -1)"
actual_revision="$(git -C "$engine_source" rev-parse HEAD)"
if [[ -z "$pinned_revision" || "$actual_revision" != "$pinned_revision" ]]; then
  echo "Engine source must be at the revision pinned in Cargo.toml: $pinned_revision" >&2
  exit 1
fi
if [[ -n "$(git -C "$engine_source" status --porcelain)" ]]; then
  echo "Engine source has uncommitted changes; package a clean pinned revision" >&2
  exit 1
fi

profile="${ANASTASIA_PACKAGE_PROFILE:-release}"
case "$profile" in
  release) profile_dir=release; suffix= ;;
  dev) profile_dir=debug; suffix=-dev ;;
  *) echo "ANASTASIA_PACKAGE_PROFILE must be release or dev" >&2; exit 1 ;;
esac
engine_target="${ANASTASIA_ENGINE_TARGET_DIR:-$engine_source/target}"
desktop_target="${ANASTASIA_DESKTOP_TARGET_DIR:-$desktop_source/target}"
ANASTASIA_CLI_BUILD_GIT_HASH="${actual_revision:0:9}" CARGO_TARGET_DIR="$engine_target" cargo build --locked --profile "$profile" --manifest-path "$engine_source/Cargo.toml" --bin anastasia
CARGO_TARGET_DIR="$desktop_target" cargo build --locked --profile "$profile" --manifest-path "$desktop_source/Cargo.toml" --bin anastasia-desktop

mkdir -p "$desktop_source/dist"
case "$(uname -s)" in
  Darwin)
    bundle="$desktop_source/dist/Anastasia$suffix.app"
    rm -rf -- "$bundle"
    mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
    cp "$desktop_target/$profile_dir/anastasia-desktop" "$bundle/Contents/MacOS/anastasia-desktop"
    cp "$engine_target/$profile_dir/anastasia" "$bundle/Contents/MacOS/anastasia-engine"
    cp "$desktop_source/LICENSE" "$desktop_source/THIRD_PARTY_NOTICES.md" "$bundle/Contents/Resources/"
    cp "$desktop_source/assets/fonts/OFL-geist.txt" "$desktop_source/assets/fonts/OFL.txt" "$desktop_source/assets/fonts/LICENSE-nerd-fonts.txt" "$bundle/Contents/Resources/"
    cat > "$bundle/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.anastasia.desktop</string>
<key>CFBundleName</key><string>Anastasia</string>
<key>CFBundleExecutable</key><string>anastasia-desktop</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>LSMinimumSystemVersion</key><string>13.0</string>
</dict></plist>
PLIST
    codesign --force --sign "${ANASTASIA_CODESIGN_IDENTITY:--}" "$bundle/Contents/MacOS/anastasia-engine"
    codesign --force --deep --sign "${ANASTASIA_CODESIGN_IDENTITY:--}" "$bundle"
    codesign --verify --deep --strict "$bundle"
    ditto -c -k --keepParent "$bundle" "$desktop_source/dist/Anastasia-macOS$suffix.zip"
    ;;
  Linux)
    package="$desktop_source/dist/Anastasia-Linux$suffix"
    mkdir -p "$package"
    cp "$desktop_target/$profile_dir/anastasia-desktop" "$package/"
    cp "$engine_target/$profile_dir/anastasia" "$package/anastasia-engine"
    cp "$desktop_source/LICENSE" "$desktop_source/THIRD_PARTY_NOTICES.md" "$package/"
    cp "$desktop_source/assets/fonts/OFL-geist.txt" "$desktop_source/assets/fonts/OFL.txt" "$desktop_source/assets/fonts/LICENSE-nerd-fonts.txt" "$package/"
    tar -C "$desktop_source/dist" -czf "$desktop_source/dist/Anastasia-Linux$suffix.tar.gz" "Anastasia-Linux$suffix"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    package="$desktop_source/dist/Anastasia-Windows$suffix"
    mkdir -p "$package"
    cp "$desktop_target/$profile_dir/anastasia-desktop.exe" "$package/"
    cp "$engine_target/$profile_dir/anastasia.exe" "$package/anastasia-engine.exe"
    cp "$desktop_source/LICENSE" "$desktop_source/THIRD_PARTY_NOTICES.md" "$package/"
    cp "$desktop_source/assets/fonts/OFL-geist.txt" "$desktop_source/assets/fonts/OFL.txt" "$desktop_source/assets/fonts/LICENSE-nerd-fonts.txt" "$package/"
    ;;
  *) echo "Unsupported host" >&2; exit 1 ;;
esac
echo "Packaged in $desktop_source/dist"
