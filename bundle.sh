#!/bin/sh
# Bouwt Sneeuw.app (zonder Dock-icoon) in target/.
set -e
cd "$(dirname "$0")"
cargo build --release
APP=target/Sneeuw.app
mkdir -p "$APP/Contents/MacOS"
cp target/release/sneeuw "$APP/Contents/MacOS/sneeuw"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Sneeuw</string>
  <key>CFBundleIdentifier</key><string>nl.timvanbergen.sneeuw</string>
  <key>CFBundleExecutable</key><string>sneeuw</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>1.0</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
codesign --force --sign - "$APP" 2>/dev/null || true
echo "Klaar: $APP"
