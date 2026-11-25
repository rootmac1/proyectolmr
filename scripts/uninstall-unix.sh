#!/usr/bin/env bash
set -euo pipefail

package_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
install_root="${LMR_INSTALL_ROOT:-$HOME/.local/opt/lmr}"
version=""
no_path=0
assume_yes=0

while (($#)); do
    case "$1" in
        --prefix)
            (($# >= 2)) || { echo "Falta ruta después de --prefix" >&2; exit 2; }
            install_root="$2"
            shift 2
            ;;
        --version)
            (($# >= 2)) || { echo "Falta versión después de --version" >&2; exit 2; }
            version="$2"
            shift 2
            ;;
        --no-path)
            no_path=1
            shift
            ;;
        --yes|-y)
            assume_yes=1
            shift
            ;;
        *) echo "Opción desconocida: $1" >&2; exit 2 ;;
    esac
done

if [[ -z "$version" ]]; then
    version_file="$package_root/VERSION"
    [[ -f "$version_file" ]] || { echo "Indica --version o ejecuta desde un paquete que contenga VERSION." >&2; exit 2; }
    version="$(tr -d '\r\n' < "$version_file")"
fi
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.+][0-9A-Za-z.-]+)?$ ]]; then
    echo "Versión inválida: $version" >&2
    exit 2
fi

[[ -d "$install_root" ]] || { echo "No se encontró la raíz de instalación: $install_root" >&2; exit 1; }
install_base="$(cd "$install_root" && pwd -P)"
[[ "$install_base" != "/" ]] || { echo "Se rechazó usar / como raíz de instalación." >&2; exit 1; }
install_path="$install_base/$version"
[[ -d "$install_path" ]] || { echo "No se encontró LMR $version en $install_path" >&2; exit 1; }
[[ ! -L "$install_path" ]] || { echo "Se rechazó retirar una instalación que es un enlace simbólico: $install_path" >&2; exit 1; }
resolved_install="$(cd "$install_path" && pwd -P)"
[[ "$resolved_install" == "$install_base/"* ]] || { echo "Se rechazó retirar una ruta fuera de la raíz de instalación: $resolved_install" >&2; exit 1; }

launcher="$HOME/.local/bin/lmr"
remove_launcher=0
if ((no_path == 0)) && [[ -f "$launcher" && -x "$launcher" && ! -L "$launcher" ]]; then
    expected_launcher="$(printf '#!/usr/bin/env bash\nexport LMR_HOME=%q\nexec "$LMR_HOME/lmr" "$@"\n' "$resolved_install")"
    actual_launcher="$(cat "$launcher")"
    launcher_lines="$(wc -l < "$launcher" | tr -d '[:space:]')"
    if [[ "$actual_launcher" == "$expected_launcher" && "$launcher_lines" == "3" ]]; then
        remove_launcher=1
    fi
fi

if ((assume_yes == 0)); then
    [[ -t 0 ]] || { echo "La confirmación interactiva no está disponible; vuelve a ejecutar con --yes." >&2; exit 2; }
    read -r -p "¿Desinstalar LMR $version de $resolved_install? [s/N] " answer
    [[ "$answer" == "s" || "$answer" == "S" || "$answer" == "si" || "$answer" == "sí" || "$answer" == "SI" || "$answer" == "Sí" ]] || { echo "Desinstalación cancelada."; exit 0; }
fi

rm -rf -- "$resolved_install"
if ((remove_launcher == 1)); then
    rm -- "$launcher"
    echo "Se retiró el lanzador que apuntaba a esta versión: $launcher"
fi
echo "LMR $version desinstalado de $resolved_install"
if ((no_path == 1)); then
    echo "No se modificó el lanzador de ~/.local/bin."
fi
