#!/bin/sh
# Builds KeyNinja.app (menu bar only, no Dock icon).
set -eu
cd "$(dirname "$0")/.."
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
cargo build --release
APP=target/KeyNinja.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/keyninja "$APP/Contents/MacOS/keyninja"
TMP=$(mktemp -d)
sips -s format png -z 512 512 assets/DonStrike.ico --out "$TMP/icon.png" >/dev/null
sips -s format icns "$TMP/icon.png" --out "$APP/Contents/Resources/AppIcon.icns" >/dev/null
rm -rf "$TMP"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>KeyNinja</string>
    <key>CFBundleDisplayName</key><string>KeyNinja</string>
    <key>CFBundleIdentifier</key><string>com.donstrike.keyninja</string>
    <key>CFBundleExecutable</key><string>keyninja</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>LSUIElement</key><true/>
</dict>
</plist>
PLIST
codesign --force --sign - "$APP" >/dev/null 2>&1 || true
echo "Done: $APP"
