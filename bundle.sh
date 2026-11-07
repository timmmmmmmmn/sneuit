#!/bin/sh
# Bouwt Sneuit.app (zonder Dock-icoon) in target/.
set -e
cd "$(dirname "$0")"
cargo build --release
APP=target/Sneuit.app
mkdir -p "$APP/Contents/MacOS"
cp target/release/sneuit "$APP/Contents/MacOS/sneuit"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Sneuit</string>
  <key>CFBundleIdentifier</key><string>nl.timvanbergen.sneuit</string>
  <key>CFBundleExecutable</key><string>sneuit</string>
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
