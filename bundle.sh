#!/bin/sh
# Bouwt Sneuit.app (zonder Dock-icoon) in target/.
set -e
cd "$(dirname "$0")"
cargo build --release
APP=target/Sneuit.app
mkdir -p "$APP/Contents/MacOS"
cp target/release/sneuit "$APP/Contents/MacOS/sneuit"

# Iconen maken uit de logo's in assets/.
ICONS=target/icons
mkdir -p "$ICONS/Sneuit.iconset" "$APP/Contents/Resources"
swift assets/make_icons.swift assets "$ICONS" >/dev/null
for s in 16 32 128 256 512; do
  sips -z $s $s "$ICONS/icon_1024.png" --out "$ICONS/Sneuit.iconset/icon_${s}x${s}.png" >/dev/null
  d=$((s * 2))
  sips -z $d $d "$ICONS/icon_1024.png" --out "$ICONS/Sneuit.iconset/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONS/Sneuit.iconset" -o "$APP/Contents/Resources/Sneuit.icns"
cp "$ICONS/menubar_18.png" "$APP/Contents/Resources/menubar.png"
cp "$ICONS/menubar_36.png" "$APP/Contents/Resources/menubar@2x.png"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Sneuit</string>
  <key>CFBundleIdentifier</key><string>nl.timvanbergen.sneuit</string>
  <key>CFBundleExecutable</key><string>sneuit</string>
  <key>CFBundleIconFile</key><string>Sneuit</string>
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
