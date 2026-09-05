#!/usr/bin/env bash
set -euo pipefail

usage() {
    printf 'Usage: %s --key KEY_PATH [TEXT]\n' "$0" >&2
    printf '       printf %%s TEXT | %s --key KEY_PATH\n' "$0" >&2
}

die() {
    printf '%s\n' "$1" >&2
    exit 1
}

key_path=''
text=''
has_text=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --key)
            [[ $# -ge 2 ]] || die '--key requires a path'
            key_path=$2
            shift 2
            ;;
        --key=*)
            key_path=${1#*=}
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        --)
            shift
            if [[ $# -gt 1 || ( $# -eq 1 && "$has_text" == true ) ]]; then
                usage
                exit 2
            fi
            if [[ $# -eq 1 ]]; then
                text=$1
                has_text=true
                shift
            fi
            ;;
        -*)
            usage
            exit 2
            ;;
        *)
            [[ "$has_text" == false ]] || { usage; exit 2; }
            text=$1
            has_text=true
            shift
            ;;
    esac
done

[[ -n "$key_path" ]] || { usage; exit 2; }
[[ -f "$key_path" ]] || die "Key file does not exist: $key_path"

key=$(tr -d '\r\n' <"$key_path")
[[ -n "$key" ]] || die 'Key file is empty'

script_directory=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repository_root=$(cd "$script_directory/.." && pwd)

if [[ "$has_text" == true ]]; then
    printf '%s' "$text" | CAMWATCH_CONFIG_KEY="$key" cargo run --quiet --manifest-path "$repository_root/Cargo.toml" -p camwatch-secret
else
    [[ -t 0 ]] && { usage; exit 2; }
    CAMWATCH_CONFIG_KEY="$key" cargo run --quiet --manifest-path "$repository_root/Cargo.toml" -p camwatch-secret
fi
