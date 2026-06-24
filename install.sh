#!/usr/bin/env bash
# Install protongen as a user-level desktop app (no sudo).
#   - binary  -> ~/.local/bin/protongen
#   - icon    -> ~/.local/share/icons/hicolor/scalable/apps/protongen.svg
#   - launcher-> ~/.local/share/applications/protongen.desktop
#
# Builds the Tauri app: the web frontend (pnpm) is bundled into the Rust binary.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bin_dir="$HOME/.local/bin"
icon_dir="$HOME/.local/share/icons/hicolor/scalable/apps"
app_dir="$HOME/.local/share/applications"

pm="pnpm"
command -v pnpm >/dev/null 2>&1 || pm="npm"

echo "==> Installing frontend dependencies ($pm)…"
( cd "$here" && $pm install )

# Build the production binary via the Tauri CLI. This is required: a plain
# `cargo build --release` leaves the app in dev mode (it would try to connect to
# the Vite dev server at localhost:1420). `tauri build` embeds the built frontend
# and flips the app to production. `beforeBuildCommand` runs `pnpm build` for us.
#
# Invoke the local CLI directly rather than `$pm run tauri build -- --no-bundle`:
# npm strips one `--`, but pnpm forwards it, which leaks `--no-bundle` through to
# `cargo build` and fails. The binary path is the same under npm and pnpm.
echo "==> Building production app (frontend + binary)…"
( cd "$here" && "$here/node_modules/.bin/tauri" build --no-bundle )

echo "==> Installing files…"
mkdir -p "$bin_dir" "$icon_dir" "$app_dir"
install -m 755 "$here/src-tauri/target/release/protongen" "$bin_dir/protongen"
install -m 644 "$here/assets/protongen.svg" "$icon_dir/protongen.svg"

cat > "$app_dir/protongen.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=protongen
GenericName=Proton Launch Command Builder
Comment=Build Steam / umu-launcher commands for CachyOS Proton
Exec=$bin_dir/protongen
Icon=protongen
Terminal=false
Categories=Game;Utility;
Keywords=proton;steam;umu;launch;cachyos;wine;
StartupWMClass=protongen
EOF
chmod 644 "$app_dir/protongen.desktop"

# Refresh caches if the tools are available (non-fatal otherwise).
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$app_dir" || true
command -v gtk-update-icon-cache  >/dev/null 2>&1 && gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" >/dev/null 2>&1 || true
command -v desktop-file-validate  >/dev/null 2>&1 && desktop-file-validate "$app_dir/protongen.desktop" || true

echo "==> Done."
echo "   Launch from your app menu (search 'protongen'),"
echo "   or run 'protongen' if ~/.local/bin is on your PATH."
case ":$PATH:" in
  *":$bin_dir:"*) ;;
  *) echo "   NOTE: $bin_dir is not on PATH — the menu shortcut still works (absolute Exec)." ;;
esac
