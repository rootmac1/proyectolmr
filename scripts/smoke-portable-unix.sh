#!/usr/bin/env bash
set -euo pipefail

if (($# != 1)); then
    echo "Uso: $0 <paquete.tar.gz>" >&2
    exit 2
fi
archive="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
temp_root="$(mktemp -d "${TMPDIR:-/tmp}/lmr-smoke.XXXXXXXX")"
cleanup() { rm -rf "$temp_root"; }
trap cleanup EXIT
package_dir="$temp_root/package"
run_dir="$temp_root/run"
install_root="$temp_root/install"
mkdir -p "$package_dir" "$run_dir"
tar -xzf "$archive" -C "$package_dir"

for required in \
    "$package_dir/CHANGELOG.md" \
    "$package_dir/docs/module-package.schema.json" \
    "$package_dir/examples/community/lmr-module.json" \
    "$package_dir/examples/community/LICENSE" \
    "$package_dir/examples/community/README.md" \
    "$package_dir/examples/community/demo.lmr"; do
    if [[ ! -s "$required" ]]; then
        echo "Falta un archivo requerido del paquete: $required" >&2
        exit 1
    fi
done

community_output="$("$package_dir/lmr" check "$package_dir/examples/community/demo.lmr" 2>&1)"
if [[ "$community_output" != *"válido"* ]]; then
    echo "El ejemplo comunitario del paquete no pasó lmr check: $community_output" >&2
    exit 1
fi

module_root="$run_dir/community-modules"
consumer_root="$run_dir/community-consumer"
mkdir -p "$module_root/modules" "$consumer_root"
cp "$package_dir/examples/community/Greeter.lmr" "$module_root/modules/Greeter.lmr"
cat > "$consumer_root/main.lmr" <<'LMR'
from Greeter import greet

begin
    writeln(greet("Portable"))
end.
LMR
community_path_output="$(LMR_MODULE_PATH="$module_root" "$package_dir/lmr" check "$consumer_root/main.lmr" 2>&1)"
if [[ "$community_path_output" != *"válido"* ]]; then
    echo "No se pudo importar un módulo comunitario desde LMR_MODULE_PATH: $community_path_output" >&2
    exit 1
fi
user_data_home="$run_dir/user-data"
mkdir -p "$user_data_home/lmr/modules"
cp "$package_dir/examples/community/Greeter.lmr" "$user_data_home/lmr/modules/Greeter.lmr"
user_module_output="$(env -u LMR_MODULE_PATH XDG_DATA_HOME="$user_data_home" "$package_dir/lmr" check "$consumer_root/main.lmr" 2>&1)"
if [[ "$user_module_output" != *"válido"* ]]; then
    echo "No se pudo importar un módulo desde la carpeta LMR del usuario: $user_module_output" >&2
    exit 1
fi

cat > "$run_dir/portable_smoke.lmr" <<'LMR'
from Math import raiz

begin
    writeln(raiz(25.0))
end.
LMR

output="$(cd "$run_dir" && "$package_dir/lmr" run "$run_dir/portable_smoke.lmr" 2>&1)"
if [[ "$output" != *"5.00"* ]]; then
    echo "El compilador portable no produjo el resultado esperado: $output" >&2
    exit 1
fi
"$package_dir/lmr" "$run_dir/portable_smoke.lmr" -c -o "$run_dir/portable_smoke.o"
if [[ ! -s "$run_dir/portable_smoke.o" ]]; then
    echo "No se generó el objeto nativo esperado" >&2
    exit 1
fi

before_checks="$(find "$run_dir" -type f | wc -l | tr -d ' ')"
"$package_dir/lmr" check "$run_dir/portable_smoke.lmr" >/dev/null
source_text="$(cat "$run_dir/portable_smoke.lmr")"
printf '%s\n' "$source_text" | "$package_dir/lmr" check --stdin "$run_dir/portable_smoke.lmr" >/dev/null
set +e
printf 'begin\nint invalid\nend\n' | "$package_dir/lmr" check --stdin "$run_dir/portable_smoke.lmr" >/dev/null 2>&1
invalid_status=$?
set -e
if ((invalid_status == 0)); then
    echo "lmr check --stdin aceptó un programa inválido" >&2
    exit 1
fi
invalid_file="$run_dir/invalid.lmr"
printf 'begin\nint invalid\nend\n' > "$invalid_file"
set +e
invalid_compile_output="$("$package_dir/lmr" "$invalid_file" 2>&1)"
invalid_compile_status=$?
set -e
if ((invalid_compile_status == 0)); then
    echo "El compilador indicó éxito con fuente inválida: $invalid_compile_output" >&2
    exit 1
fi
rm "$invalid_file"
after_checks="$(find "$run_dir" -type f | wc -l | tr -d ' ')"
if [[ "$before_checks" != "$after_checks" ]]; then
    echo "lmr check generó archivos inesperados" >&2
    exit 1
fi

"$package_dir/install-unix.sh" --prefix "$install_root" --no-path
version="$(tr -d '\r\n' < "$package_dir/VERSION")"
version_output="$("$install_root/$version/lmr" --version)"
if [[ "$version_output" != *"$version"* ]]; then
    echo "El compilador instalado no informa su versión: $version_output" >&2
    exit 1
fi

path_home="$temp_root/user-home"
mkdir -p "$path_home"
old_version="0.1.0-alpha.9"
path_install_root="$(mkdir -p "$temp_root/path-install" && cd "$temp_root/path-install" && pwd -P)"
old_install="$path_install_root/$old_version"
mkdir -p "$old_install" "$path_home/.local/bin"
cp "$package_dir/lmr" "$old_install/lmr"
chmod +x "$old_install/lmr"
printf '%s' "$old_version" > "$old_install/VERSION"
old_home_line="export LMR_HOME=$(printf '%q' "$old_install")"
printf '#!/usr/bin/env bash\n%s\nexec "$LMR_HOME/lmr" "$@"\n' "$old_home_line" > "$path_home/.local/bin/lmr"
chmod +x "$path_home/.local/bin/lmr"
HOME="$path_home" "$package_dir/install-unix.sh" --prefix "$path_install_root"
new_home_line="export LMR_HOME=$(printf '%q' "$path_install_root/$version")"
if ! grep -Fqx -- "$new_home_line" "$path_home/.local/bin/lmr"; then
    echo "El instalador no actualizó el lanzador LMR existente" >&2
    exit 1
fi
path_output="$(HOME="$path_home" "$path_home/.local/bin/lmr" --version)"
if [[ "$path_output" != *"$version"* ]]; then
    echo "El lanzador de PATH no encontró el runtime instalado: $path_output" >&2
    exit 1
fi
installed_output="$(cd "$run_dir" && HOME="$path_home" "$path_home/.local/bin/lmr" run "$run_dir/portable_smoke.lmr" 2>&1)"
if [[ "$installed_output" != *"5.00"* ]]; then
    echo "El lanzador instalado no encontró módulos/runtime: $installed_output" >&2
    exit 1
fi
HOME="$path_home" "$package_dir/uninstall-unix.sh" --prefix "$path_install_root" --yes
if [[ -e "$path_install_root/$version" || -e "$path_home/.local/bin/lmr" ]]; then
    echo "El desinstalador no retiró la instalación y su lanzador" >&2
    exit 1
fi
if [[ ! -d "$old_install" ]]; then
    echo "La actualización eliminó una versión anterior que debía conservar" >&2
    exit 1
fi
foreign_home="$temp_root/foreign-home"
foreign_root="$temp_root/foreign-install"
mkdir -p "$foreign_home/.local/bin"
printf '#!/usr/bin/env sh\necho foreign-command\n' > "$foreign_home/.local/bin/lmr"
chmod +x "$foreign_home/.local/bin/lmr"
set +e
foreign_install_output="$(HOME="$foreign_home" "$package_dir/install-unix.sh" --prefix "$foreign_root" 2>&1)"
foreign_install_status=$?
set -e
if ((foreign_install_status == 0)) || [[ "$(HOME="$foreign_home" "$foreign_home/.local/bin/lmr")" != "foreign-command" ]]; then
    echo "El instalador reemplazó un comando ajeno: $foreign_install_output" >&2
    exit 1
fi
"$package_dir/uninstall-unix.sh" --prefix "$install_root" --no-path --yes
if [[ -e "$install_root/$version" ]]; then
    echo "El desinstalador no retiró la instalación sin lanzador" >&2
    exit 1
fi
echo "Smoke correcto: paquete, checks, actualización del lanzador e instalación/desinstalación funcionaron fuera del checkout."
