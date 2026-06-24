#!/usr/bin/env bash
# Remove the user-level protongen install.
set -euo pipefail

rm -f "$HOME/.local/bin/protongen"
rm -f "$HOME/.local/share/icons/hicolor/scalable/apps/protongen.svg"
rm -f "$HOME/.local/share/applications/protongen.desktop"

command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$HOME/.local/share/applications" || true
command -v gtk-update-icon-cache  >/dev/null 2>&1 && gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" >/dev/null 2>&1 || true

echo "protongen uninstalled. (Your ~/.config/protongen/params.toml override, if any, was left intact.)"
