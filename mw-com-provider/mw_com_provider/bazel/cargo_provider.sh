#!/usr/bin/env bash
set -euo pipefail

if [[ -n "${BUILD_WORKSPACE_DIRECTORY:-}" ]]; then
    cd "$BUILD_WORKSPACE_DIRECTORY"
else
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    cd "$script_dir/.."
fi

provider_root=""
if [[ -f "xtask/Cargo.toml" ]]; then
    provider_root="$PWD"
elif [[ -f "mw_com_provider/xtask/Cargo.toml" ]]; then
    provider_root="$PWD/mw_com_provider"
else
    echo "failed to locate mw_com_provider/xtask/Cargo.toml from $PWD" >&2
    exit 2
fi

command="${1:-}"
if [[ $# -gt 0 ]]; then
    shift
fi

case "$command" in
    check)
        exec cargo check -p mw_com_provider "$@"
        ;;
    test)
        exec cargo test -p mw_com_provider "$@"
        ;;
    codegen)
        exec cargo run --manifest-path "$provider_root/xtask/Cargo.toml" -- codegen "$@"
        ;;
    verify-codegen)
        exec cargo run --manifest-path "$provider_root/xtask/Cargo.toml" -- verify-codegen "$@"
        ;;
    *)
        echo "usage: $0 {check|test|codegen|verify-codegen} [args...]" >&2
        exit 2
        ;;
esac