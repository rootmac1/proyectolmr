#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="$(sed -n 's/^version = "\([^"]*\)"$/\1/p' "$repo_root/Cargo.toml" | head -n 1)"
if [[ -z "$version" ]]; then
    echo "No se pudo leer la versión de Cargo.toml" >&2
    exit 1
fi

case "$(uname -s)" in
    Linux*) os="linux" ;;
    Darwin*) os="macos" ;;
    *) echo "Sistema no soportado por este empaquetador: $(uname -s)" >&2; exit 1 ;;
esac
case "$(uname -m)" in
    x86_64|amd64) arch="x86_64" ;;
    arm64|aarch64) arch="arm64" ;;
    *) arch="$(uname -m)" ;;
esac

dist_dir="$repo_root/dist"
package_name="lmr-$version-$os-$arch"
archive="$dist_dir/$package_name.tar.gz"
mkdir -p "$dist_dir"
if [[ -e "$archive" ]]; then
    echo "El archivo ya existe; muévelo manualmente antes de empaquetar: $archive" >&2
    exit 1
fi

stage="$(mktemp -d "${TMPDIR:-/tmp}/lmr-package.XXXXXXXX")"
cleanup() { rm -rf "$stage"; }
trap cleanup EXIT

cd "$repo_root"
cargo build --locked --release
binary="$repo_root/target/release/lmr"
if [[ ! -f "$binary" ]]; then
    echo "No se encontró el compilador generado: $binary" >&2
    exit 1
fi

mkdir -p "$stage/modules" "$stage/src" "$stage/docs" "$stage/examples/community" "$stage/scripts"
cp "$binary" "$stage/lmr"
for module in "$repo_root"/modules/*; do
    [[ -f "$module" ]] || continue
    [[ "$(basename "$module")" == "TEMPLATE.lmr" ]] && continue
    cp "$module" "$stage/modules/"
done
cp "$repo_root/src/trade_bridge.c" "$stage/src/trade_bridge.c"
cp "$repo_root/README.md" "$repo_root/CONTRIBUTING.md" "$repo_root/LICENSE" "$repo_root/VERSIONING.md" "$repo_root/CHANGELOG.md" "$repo_root/GUIA_LMR_ES.md" "$stage/"
cp "$repo_root/docs/MODULE_API.md" "$stage/docs/"
cp "$repo_root/docs/module-package.schema.json" "$stage/docs/"
cp "$repo_root/examples/community/"* "$stage/examples/community/"
cp "$repo_root/scripts/validate-module-package.js" "$stage/scripts/"
cp "$repo_root/scripts/install-unix.sh" "$repo_root/scripts/uninstall-unix.sh" "$stage/"
printf '%s' "$version" > "$stage/VERSION"
chmod +x "$stage/lmr" "$stage/install-unix.sh" "$stage/uninstall-unix.sh"
tar -czf "$archive" -C "$stage" .
echo "Paquete portable creado: $archive"
echo "Requiere Clang en PATH para compilar programas .lmr."
