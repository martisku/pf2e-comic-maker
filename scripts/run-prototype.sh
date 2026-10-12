#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "$0")/.."
project_root="$PWD"
if [[ -n "${COMIC_TOOLCHAIN_DIR:-}" ]]; then
    export CARGO_HOME="$COMIC_TOOLCHAIN_DIR/cargo"
    export RUSTUP_HOME="$COMIC_TOOLCHAIN_DIR/rustup"
    export XDG_CACHE_HOME="$COMIC_TOOLCHAIN_DIR/cache"
    export XDG_DATA_HOME="$COMIC_TOOLCHAIN_DIR/data"
    export XDG_CONFIG_HOME="$COMIC_TOOLCHAIN_DIR/config"
    export PATH="$COMIC_TOOLCHAIN_DIR/cargo/bin:$COMIC_TOOLCHAIN_DIR/bin:$PATH"
    for tool_bin in "$XDG_DATA_HOME/.dx/tools/"*/bin "$XDG_DATA_HOME/.dx/tools/"*; do
        if [[ -d "$tool_bin" ]]; then export PATH="$tool_bin:$PATH"; fi
    done
    export NO_DOWNLOADS=1
fi
cd crates/web
dx build --web --release --debug-symbols false --locked -p comic-web
cd "$project_root"
export COMIC_PUBLIC_DIR="$project_root/target/dx/comic-web/release/web/public"
if [[ ! -f "$COMIC_PUBLIC_DIR/index.html" ]]; then
    printf '%s\n' "Dioxus output was not found at $COMIC_PUBLIC_DIR" >&2
    exit 1
fi
cargo run --locked -p comic-server
