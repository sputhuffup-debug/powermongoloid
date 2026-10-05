#!/bin/bash
# Installs the system libraries Bevy needs to build on Linux, so cloud
# sessions can run cargo build/test/clippy without manual setup.
set -euo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

pkgs="pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev mesa-vulkan-drivers xvfb libxkbcommon-x11-0 libxcursor1 libxi6 libxrandr2"
missing=""
for p in $pkgs; do
  dpkg -s "$p" >/dev/null 2>&1 || missing="$missing $p"
done
if [ -n "$missing" ]; then
  apt-get update -qq
  DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends $missing
fi

cd "$CLAUDE_PROJECT_DIR"
cargo fetch --quiet
