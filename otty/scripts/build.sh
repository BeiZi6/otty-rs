#!/usr/bin/env bash
# Build everything, for the host by default.
#
#   ./scripts/build.sh                 release build of the workspace
#   ./scripts/build.sh --debug         a debug build
#   ./scripts/build.sh --target <t>    cross-compile to one target triple
#
# The target directory defaults to `target/` but honours $CARGO_TARGET_DIR, which
# matters here: a full workspace build is about 6 GB, and pointing it at another
# volume is the difference between a build and a disk-full error.

set -euo pipefail

here="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$here"

profile="release"
target_args=()

while [ $# -gt 0 ]; do
    case "$1" in
        --debug) profile="debug"; shift ;;
        --target) target_args+=(--target "$2"); shift 2 ;;
        -h|--help)
            sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'
            exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

cargo_args=()
[ "$profile" = "release" ] && cargo_args+=(--release)

echo "==> building the workspace ($profile${target_args[*]:+ ${target_args[*]}})"
cargo build --workspace "${cargo_args[@]}" "${target_args[@]}"

echo "==> running the test suite"
if [ "$profile" = "release" ]; then
    cargo test --workspace --release "${target_args[@]}"
else
    cargo test --workspace "${target_args[@]}"
fi

echo "==> done"
echo "    otty      ${CARGO_TARGET_DIR:-$here/target}/$profile/otty"
echo "    otty-bridge  ${CARGO_TARGET_DIR:-$here/target}/$profile/otty-bridge  (frontend/electron runs it)"
