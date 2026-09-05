#!/usr/bin/env bash
set -euo pipefail

usage() {
    printf 'Usage: %s OUTPUT_PATH\n' "$0" >&2
}

if [[ $# -ne 1 ]]; then
    usage
    exit 2
fi

key_path=$1
key_directory=$(dirname "$key_path")
key_name=$(basename "$key_path")

if [[ ! -d "$key_directory" ]]; then
    printf 'Key directory does not exist: %s\n' "$key_directory" >&2
    exit 1
fi

if [[ -e "$key_path" ]]; then
    printf 'Refusing to overwrite existing key: %s\n' "$key_path" >&2
    exit 1
fi

if ! command -v openssl >/dev/null 2>&1; then
    printf 'openssl is required\n' >&2
    exit 1
fi

umask 077
temporary_path=$(mktemp "$key_directory/.${key_name}.XXXXXX")
trap 'rm -f "$temporary_path"' EXIT

openssl rand -base64 32 >"$temporary_path"
mv "$temporary_path" "$key_path"
trap - EXIT

printf 'Generated a 32-byte Base64 key at %s\n' "$key_path"
