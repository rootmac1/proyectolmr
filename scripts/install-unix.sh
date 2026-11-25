#!/usr/bin/env bash
set -euo pipefail

package_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
version_file="$package_root/VERSION"
if [[ ! -f "$version_file" ]]; then
    echo "No se encontró VERSION. Ejecuta este instalador desde el paquete LMR extraído." >&2
    exit 1
fi
version="$(tr -d '\r\n' < "$version_file")"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.+][0-9A-Za-z.-]+)?$ ]]; then
    echo "Versión inválida: $version" >&2
    exit 1
fi

install_root="${LMR_INSTALL_ROOT:-$HOME/.local/opt/lmr}"
no_path=0
while (($#)); do
    case "$1" in
        --prefix)
            (($# >= 2)) || { echo "Falta ruta después de --prefix" >&2; exit 2; }
            install_root="$2"
            shift 2
            ;;
        --no-path)
            no_path=1
            shift
            ;;
        *) echo "Opción desconocida: $1" >&2; exit 2 ;;
    esac
done

install_root="$(mkdir -p "$install_root" && cd "$install_root" && pwd -P)"
install_path="$install_root/$version"
if [[ -e "$install_path" ]]; then
    echo "Esta versión ya existe: $install_path" >&2
    exit 1
fi

bin_dir="$HOME/.local/bin"
link_path="$bin_dir/lmr"
managed_launcher=0
launcher_matches_install_root() {
    local candidate candidate_version installed_version resolved_candidate expected_launcher actual_launcher line_count
    [[ -f "$link_path" && -x "$link_path" && ! -L "$link_path" ]] || return 1
    for candidate in "$install_root"/*; do
        [[ -d "$candidate" && ! -L "$candidate" ]] || continue
        candidate_version="$(basename "$candidate")"
        [[ "$candidate_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.+][0-9A-Za-z.-]+)?$ ]] || continue
        [[ -x "$candidate/lmr" && -f "$candidate/VERSION" ]] || continue
        installed_version="$(tr -d '\r\n' < "$candidate/VERSION")"
        [[ "$installed_version" == "$candidate_version" ]] || continue
        resolved_candidate="$(cd "$candidate" && pwd -P)"
        expected_launcher="$(printf '#!/usr/bin/env bash\nexport LMR_HOME=%q\nexec "$LMR_HOME/lmr" "$@"\n' "$resolved_candidate")"
        actual_launcher="$(cat "$link_path")"
        line_count="$(wc -l < "$link_path" | tr -d '[:space:]')"
        if [[ "$actual_launcher" == "$expected_launcher" && "$line_count" == "3" ]]; then
            return 0
        fi
    done
    return 1
}

if ((no_path == 0)) && [[ -e "$link_path" || -L "$link_path" ]]; then
    if launcher_matches_install_root; then
        managed_launcher=1
    else
        echo "Ya existe '$link_path' y no pertenece a una instalación LMR de esta raíz; no se reemplazó." >&2
        exit 1
    fi
fi

mkdir -p "$install_path"
cp -R "$package_root"/. "$install_path"/
chmod +x "$install_path/lmr"

if ((no_path == 0)); then
    mkdir -p "$bin_dir"
    link_temp="$(mktemp "$bin_dir/.lmr.XXXXXXXX")"
    cleanup_link_temp() { [[ -z "${link_temp:-}" ]] || rm -f -- "$link_temp"; }
    trap cleanup_link_temp EXIT
    printf '#!/usr/bin/env bash\nexport LMR_HOME=%q\nexec "$LMR_HOME/lmr" "$@"\n' "$install_path" > "$link_temp"
    chmod +x "$link_temp"
    mv -f -- "$link_temp" "$link_path"
    link_temp=""
    if ((managed_launcher == 1)); then
        echo "Se actualizó el lanzador de LMR en $link_path."
    fi
    case ":$PATH:" in
        *":$bin_dir:"*) ;;
        *)
            echo "Se creó $link_path. Agrega '$bin_dir' al PATH de tu shell para ejecutar 'lmr'."
            ;;
    esac
fi

echo "LMR $version instalado en $install_path"
if ((no_path == 1)); then
    echo "No se creó enlace en ~/.local/bin. Ejecuta: $install_path/lmr --version"
else
    echo "Prueba la instalación con: $install_path/lmr --version"
fi
