#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
requested_output="${1:-$repo_root/dist}"
output_dir="$(mkdir -p "$requested_output" && cd "$requested_output" && pwd -P)"

package_extension() {
    local extension_dir="$1"
    local package_name="$2"
    local package_json="$repo_root/$extension_dir/package.json"
    local version
    version="$(node -p "require(process.argv[1]).version" "$package_json")"
    if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.+][0-9A-Za-z.-]+)?$ ]]; then
        echo "Versión inválida en $package_json: $version" >&2
        exit 1
    fi

    local output="$output_dir/$package_name-$version.vsix"
    if [[ -e "$output" ]]; then
        echo "El VSIX ya existe; usa una carpeta de salida vacía: $output" >&2
        exit 1
    fi
    (
        cd "$repo_root/$extension_dir"
        npx --yes @vscode/vsce@4.0.0 package --no-dependencies --allow-missing-repository --out "$output"
    )
    test -s "$output"
    unzip -t "$output" >/dev/null
    echo "VSIX válido: $output"
}

package_extension "lmr-vscode" "lmr-language-support"
package_extension "antigravity/lmr-file-icons" "lmr-file-icons"
