#!/bin/sh
# Installeert Sneuit in /Applications en laat hem automatisch starten bij het inloggen.
set -e
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"

./bundle.sh
APP=/Applications/Sneuit.app
AGENT="$HOME/Library/LaunchAgents/nl.timvanbergen.sneuit.plist"

# Oude versie stoppen en vervangen.
launchctl bootout "gui/$(id -u)" "$AGENT" 2>/dev/null || true
pkill -f "Sneuit.app/Contents/MacOS/sneuit" 2>/dev/null || true
rm -rf "$APP"
cp -R target/Sneuit.app "$APP"

# Login-agent: start Sneuit bij het inloggen. Stoppen via het menu blijft gewoon werken.
mkdir -p "$HOME/Library/LaunchAgents"
cat > "$AGENT" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>nl.timvanbergen.sneuit</string>
  <key>ProgramArguments</key>
  <array><string>$APP/Contents/MacOS/sneuit</string></array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><false/>
  <key>ProcessType</key><string>Interactive</string>
</dict>
</plist>
PLIST
launchctl bootstrap "gui/$(id -u)" "$AGENT"
echo "Sneuit is geïnstalleerd in $APP en start voortaan automatisch bij het inloggen."
