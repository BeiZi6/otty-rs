#!/usr/bin/env bash
# Build the four `otty-helper` binaries the app ships.
#
# The shipped Otty 1.5.4 bundle carries exactly these, under
# `Contents/Resources/helper/<triple>/otty-helper`:
#
#     aarch64-apple-darwin          aarch64-unknown-linux-musl
#     x86_64-apple-darwin           x86_64-unknown-linux-musl
#
# The helper is uploaded to a remote host by the SSH integration, so the Linux
# builds are **musl** on purpose: a static binary runs on any distribution the
# user's server happens to be. The macOS builds are for the local side, where the
# host is the user's own machine.
#
# A cross-linker is needed for everything that is not the host. On macOS the
# Linux targets come from the `messense/macos-cross-toolchains` tarballs; on
# Linux, `musl-gcc` covers x86_64 and `aarch64-linux-musl-gcc` the rest.

set -euo pipefail

here="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$here"

triples=(
    aarch64-apple-darwin
    x86_64-apple-darwin
    aarch64-unknown-linux-musl
    x86_64-unknown-linux-musl
)

out="${1:-dist/helpers}"
mkdir -p "$out"

host="$(rustc -vV | sed -n 's/^host: //p')"

for triple in "${triples[@]}"; do
    echo "==> otty-helper for $triple"
    if ! rustup target list --installed | grep -qx "$triple"; then
        echo "    target not installed; run: rustup target add $triple" >&2
        continue
    fi

    # A musl target needs an explicit linker; the darwin ones only link on macOS.
    case "$triple" in
        *-unknown-linux-musl)
            if [ "$host" = "$triple" ]; then
                :
            elif [ "$triple" = "x86_64-unknown-linux-musl" ]; then
                export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER="${MUSL_GCC:-musl-gcc}"
            else
                export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER="${AARCH64_MUSL_GCC:-aarch64-linux-musl-gcc}"
            fi
            ;;
        *-apple-darwin)
            if [ "$(uname -s)" != "Darwin" ]; then
                echo "    skipping: darwin targets link only on macOS" >&2
                continue
            fi
            ;;
    esac

    cargo build -p otty-helper --release --target "$triple"
    dir="${CARGO_TARGET_DIR:-$here/target}/$triple/release"
    mkdir -p "$out/$triple"
    cp "$dir/otty-helper" "$out/$triple/otty-helper"

    # A record of what each helper is, so a support report can be answered.
    if command -v file >/dev/null 2>&1; then
        file "$out/$triple/otty-helper" | tee "$out/$triple/otty-helper.info"
    fi
done

echo
echo "==> helpers written to $out"
find "$out" -type f -name 'otty-helper' -exec ls -l {} \;
