#!/bin/sh
# Verwijdert Sneuit en de automatische start.
AGENT="$HOME/Library/LaunchAgents/nl.timvanbergen.sneuit.plist"
launchctl bootout "gui/$(id -u)" "$AGENT" 2>/dev/null || true
pkill -f "Sneuit.app/Contents/MacOS/sneuit" 2>/dev/null || true
rm -f "$AGENT"
rm -rf /Applications/Sneuit.app
echo "Sneuit is verwijderd."
