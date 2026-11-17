#!/usr/bin/env bash
# Builds and checks both parts of Touhou Mobs, installing nothing:
#   target/thmobs.hmod           content (mob types, textures, spawn eggs, danmaku sheets)
#   target/thmobs_native.hnmod   native code (built into the game by the launcher)
# Runs the native unit tests against the pinned Horizon checkout first; they write mobs.json, which
# gen_content.py builds the content from.
set -euo pipefail
. "$HOME/.cargo/env"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HORIZON="${HORIZON:-$HOME/horizon-dev/wt-census}"          # detached checkout of the pinned revision
TARGET="${CHECK_TARGET:-$HOME/horizon-dev/check-target-pinned}"
TOOL="${HORIZON_MOD_TOOL:-$HOME/horizon-dev/forks/Lunatic/target/tools/debug/cargo-horizon-mod}"
JARS="${JARS:-$HOME/horizon-dev/th-ports/jars}"             # the original mods' jars, unpacked
PACK="${PACKNATIVE:-$HOME/horizon-dev/natures-compass/tools/packnative.py}"
mkdir -p "$ROOT/target"

want=$(sed -n 's/^horizon_revision = "\(.*\)"/\1/p' "$ROOT/native/horizon-mixin.toml")
have=$(git -C "$HORIZON" rev-parse HEAD)
[ "$want" = "$have" ] || { echo "Horizon checkout is $have, the mod is pinned to $want" >&2; exit 1; }

echo "== native unit tests (Horizon $have)"
cd "$ROOT/native/api"
cp "$HORIZON/Cargo.lock" Cargo.lock
trap 'rm -f "$ROOT/native/api/Cargo.lock"' EXIT
cargo --config "patch.\"https://github.com/horizon-rust/horizon\".horizon-client.path=\"$HORIZON/crates/horizon-client\"" \
      --config "build.target-dir=\"$TARGET\"" test --lib -j 2
rm -f Cargo.lock

echo "== content"
python3 -I "$ROOT/gen_content.py" "$JARS" "$ROOT/target/content" "$ROOT/mobs.json"
cd "$ROOT/target/content"
"$TOOL" horizon-mod build
"$TOOL" horizon-mod check target/horizon-mod/thmobs.hmod
cp target/horizon-mod/thmobs.hmod "$ROOT/target/"

echo "== native bundle"
python3 -I "$PACK" "$ROOT/native" "$ROOT/target/thmobs_native.hnmod"
"$TOOL" horizon-mod check "$ROOT/target/thmobs_native.hnmod"
sha256sum "$ROOT/target/"*.hmod "$ROOT/target/"*.hnmod
