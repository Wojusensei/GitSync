#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:-}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION=$(node -p "require('./package.json').version")
ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
  ARCH="aarch64"
fi

APP_DIR="src-tauri/target"
if [ -n "$TARGET" ]; then
  APP_DIR="$APP_DIR/$TARGET"
fi
APP_PATH="$APP_DIR/release/bundle/macos/GitSync.app"
BUNDLE_DIR="$APP_DIR/release/bundle"

ARGS=(--ci --bundles app)
if [ -n "$TARGET" ]; then
  ARGS+=(--target "$TARGET")
fi

SIGNED=0
# 公证凭据先转存为 NOTARY_*：下方 unset APPLE_ID 等是为了避免 Tauri CLI
# 在 build 时自动公证，凭据留给本脚本最后统一使用。
# 此前实现先 unset 再检查同名变量，导致配置了 secrets 也必然在公证前退出。
NOTARY_APPLE_ID="${APPLE_ID:-}"
NOTARY_APPLE_PASSWORD="${APPLE_PASSWORD:-}"
NOTARY_APPLE_TEAM_ID="${APPLE_TEAM_ID:-}"
if [ -n "${APPLE_CERTIFICATE:-}" ]; then
  SIGNED=1
  export APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD
  if [ -n "${APPLE_SIGNING_IDENTITY:-}" ]; then
    export APPLE_SIGNING_IDENTITY
  else
    unset APPLE_SIGNING_IDENTITY
  fi
  unset APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID
else
  unset APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD APPLE_SIGNING_IDENTITY APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID
fi

npm run tauri build -- "${ARGS[@]}"

if [ "$SIGNED" -eq 0 ]; then
  echo "::warning::未配置 Apple secrets，使用 ad-hoc 签名；DMG 可打开但会提示未验证开发者"
  codesign --force --deep --sign - "$APP_PATH"
fi

codesign --verify --deep --strict --verbose=2 "$APP_PATH"

# 公证 .app 本体（Apple 推荐）：先公证并 staple，用户把应用从 DMG
# 复制出来后离线也能通过 Gatekeeper 校验
if [ "$SIGNED" -eq 1 ]; then
  if [ -z "$NOTARY_APPLE_ID" ] || [ -z "$NOTARY_APPLE_PASSWORD" ] || [ -z "$NOTARY_APPLE_TEAM_ID" ]; then
    echo "::error::APPLE_CERTIFICATE 已配置，但缺少 APPLE_ID / APPLE_PASSWORD / APPLE_TEAM_ID，无法公证"
    exit 1
  fi
  APP_ZIP="$BUNDLE_DIR/macos/GitSync-notarize.zip"
  ditto -c -k --keepParent "$APP_PATH" "$APP_ZIP"
  xcrun notarytool submit "$APP_ZIP" \
    --apple-id "$NOTARY_APPLE_ID" \
    --password "$NOTARY_APPLE_PASSWORD" \
    --team-id "$NOTARY_APPLE_TEAM_ID" \
    --wait
  rm -f "$APP_ZIP"
  xcrun stapler staple "$APP_PATH"
  xcrun stapler validate "$APP_PATH"
fi

STAGING=$(mktemp -d)
trap 'rm -rf "$STAGING"' EXIT
ditto "$APP_PATH" "$STAGING/GitSync.app"
ln -s /Applications "$STAGING/Applications"
mkdir -p "$BUNDLE_DIR/dmg"
DMG="$BUNDLE_DIR/dmg/GitSync_${VERSION}_${ARCH}.dmg"
hdiutil create -volname GitSync -srcfolder "$STAGING" -ov -format UDZO -imagekey zlib-level=9 "$DMG"
rm -rf "$STAGING"
trap - EXIT

# DMG 同样公证并 staple（凭据已在应用公证前校验过）
if [ "$SIGNED" -eq 1 ]; then
  xcrun notarytool submit "$DMG" \
    --apple-id "$NOTARY_APPLE_ID" \
    --password "$NOTARY_APPLE_PASSWORD" \
    --team-id "$NOTARY_APPLE_TEAM_ID" \
    --wait
  xcrun stapler staple "$DMG"
  xcrun stapler validate "$DMG"
fi

echo "DMG: $DMG"
