#!/bin/sh
# Installeert Sneeuw in /Applications en laat hem automatisch starten bij het inloggen.
set -e
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"

./bundle.sh
APP=/Applications/Sneeuw.app
AGENT="$HOME/Library/LaunchAgents/nl.timvanbergen.sneeuw.plist"

# Oude versie stoppen en vervangen.
launchctl bootout "gui/$(id -u)" "$AGENT" 2>/dev/null || true
pkill -f "Sneeuw.app/Contents/MacOS/sneeuw" 2>/dev/null || true
rm -rf "$APP"
cp -R target/Sneeuw.app "$APP"

# Login-agent: start Sneeuw bij het inloggen. Stoppen via het menu blijft gewoon werken.
mkdir -p "$HOME/Library/LaunchAgents"
cat > "$AGENT" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>nl.timvanbergen.sneeuw</string>
  <key>ProgramArguments</key>
  <array><string>$APP/Contents/MacOS/sneeuw</string></array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><false/>
  <key>ProcessType</key><string>Interactive</string>
</dict>
</plist>
PLIST
launchctl bootstrap "gui/$(id -u)" "$AGENT"
echo "Sneeuw is geïnstalleerd in $APP en start voortaan automatisch bij het inloggen."
