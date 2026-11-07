#!/bin/sh
# Verwijdert Sneeuw en de automatische start.
AGENT="$HOME/Library/LaunchAgents/nl.timvanbergen.sneeuw.plist"
launchctl bootout "gui/$(id -u)" "$AGENT" 2>/dev/null || true
pkill -f "Sneeuw.app/Contents/MacOS/sneeuw" 2>/dev/null || true
rm -f "$AGENT"
rm -rf /Applications/Sneeuw.app
echo "Sneeuw is verwijderd."
