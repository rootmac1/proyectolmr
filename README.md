# LMR

[![CI](https://github.com/rootmac1/proyectolmr/actions/workflows/windows.yml/badge.svg?branch=main)](https://github.com/rootmac1/proyectolmr/actions/workflows/windows.yml)

LMR es un lenguaje de programación de sintaxis estructurada y tipado estático. Su compilador está escrito en Rust y traduce programas LMR a LLVM IR; Clang produce el ejecutable nativo.

> **Estado:** versión `0.1.0-alpha.10`. Es un proyecto en desarrollo. La sintaxis, el runtime y los módulos pueden cambiar; los paquetes portables están en fase preliminar y todavía no son una publicación estable.

Repositorio y guía del proyecto: [GitHub](https://github.com/rootmac1/proyectolmr).

## Requisitos para compilar desde el repositorio

- Rust estable con soporte para la edición 2024 (Rust 1.85 o posterior).
- Clang accesible como `clang` desde `PATH`, para enlazar programas LMR.
- En Windows, las bibliotecas del toolchain que usa el enlace del proyecto. Para incrustar el icono durante la compilación también se recomienda `llvm-rc`; sin él, el build puede continuar sin el icono.
- `curl` debe estar disponible en `PATH` si el programa usa módulos de red como `Trade` o `Setka`.

## Compilar el compilador

```powershell
cargo build --release
```

El ejecutable se genera en `target\release\lmr.exe` en Windows o `target/release/lmr` en Linux/macOS.

## Ejecutar un programa LMR

Desde la raíz del repositorio:

```powershell
cargo run -- run .\demo.lmr
```

O, después de compilar el compilador:

```powershell
.\target\release\lmr.exe run .\demo.lmr
```

La CLI también acepta `lmr archivo.lmr`, `lmr archivo.lmr -o salida.exe` y `lmr archivo.lmr -c`. Ejecutar sin argumentos inicia el REPL. Consulta `lmr --help` para la ayuda disponible.

Para instalar el ZIP portable en Windows para el usuario actual, extrae el archivo y ejecuta `install-windows.ps1`. El script instala en `%LOCALAPPDATA%\Programs\LMR\<versión>` y pone esa versión al inicio del PATH de usuario; al actualizar conserva los archivos de versiones anteriores, pero deja la nueva como comando predeterminado. Usa `-NoPath` para instalar sin modificar PATH. En PowerShell puedes usar `-PathScope Process` para cambiar solo el PATH de esa sesión; usa `& .\install-windows.ps1 -PathScope Process` para conservar el cambio en la sesión actual. Para quitar la versión que corresponde al ZIP, ejecuta `uninstall-windows.ps1`; admite `-Version`, `-NoPath`, `-PathScope Process` y `-WhatIf`. Abre una terminal nueva para que otras aplicaciones reciban el PATH de usuario actualizado.

Desde el checkout del repositorio, en Linux/macOS genera el paquete nativo con `bash scripts/package-unix.sh`; tras extraer el `.tar.gz`, ejecuta `./install-unix.sh`. Por defecto instala en `~/.local/opt/lmr/<versión>` y crea un lanzador en `~/.local/bin/lmr`. Si actualizas desde la misma raíz de instalación, reemplaza solo un lanzador LMR reconocido y conserva las carpetas de versiones anteriores. Usa `--no-path` para omitir el lanzador o `--prefix <ruta>` para elegir la raíz de instalación. Para desinstalar la versión del paquete extraído, ejecuta `./uninstall-unix.sh`; usa `--version <versión>` para otra versión. El script pide confirmación y solo elimina el lanzador si apunta a esa versión. Usa `--yes` en automatización. Clang debe estar en `PATH` para compilar programas.

## Qué hay en el repositorio

- `src/`: lexer, parser, análisis semántico, generador LLVM y puente nativo.
- `modules/`: módulos LMR incluidos, más [`manifest.json`](modules/manifest.json), catálogo descriptivo.
- `docs/MODULE_API.md`: contrato actual para documentar y contribuir módulos.
- `docs/module-package.schema.json`: esquema del manifiesto comunitario `lmr-module.json`.
- `scripts/validate-module-package.js`: validador local del contrato para paquetes comunitarios.
- `scripts/validate-release-metadata.js`: comprueba que Cargo, README, política de versiones y changelog coincidan.
- `scripts/extract-release-notes.js`: prepara las notas del release desde la sección correspondiente del changelog.
- `examples/community/`: ejemplo de módulo distribuido como paquete comunitario.
- `docs/ROADMAP_PUBLIC_RELEASE.md`: etapas y estado de preparación para publicación.
- `CHANGELOG.md`: resumen de cambios por versión.
- `.github/workflows/windows.yml`: formato, pruebas, empaquetado y smoke test del ZIP en Windows.
- `.github/workflows/release.yml`: prepara paquetes por plataforma y publica los assets al crear una etiqueta `v<versión>`.
- `docs/RELEASE_PROCESS.md`: pasos para preparar, etiquetar y verificar una publicación.
- `scripts/validate-module-manifest.ps1`: comprueba que el catálogo coincide con los módulos fuente.
- `lmr-vscode/`: extensión para resaltado, snippets, autocompletado del catálogo y diagnósticos.
- `antigravity/lmr-file-icons/`: tema opcional de iconos `.lmr` para Antigravity y editores compatibles; el release genera un VSIX separado.
- `lmr check archivo.lmr`: valida sintaxis, tipos e imports sin invocar Clang ni generar ejecutables.
- [`GUIA_LMR_ES.md`](GUIA_LMR_ES.md): guía de aprendizaje y referencia del lenguaje.
- `demo.lmr`, `demo_*.lmr` y `pong.lmr`: programas de ejemplo independientes.

Importa módulos desde los archivos del proyecto, por ejemplo:

```lmr
from Math import raiz

begin
    writeln(raiz(25.0))
end.
```

Los módulos comunitarios se pueden colocar en `%LOCALAPPDATA%\LMR\modules` (Windows) o en `$XDG_DATA_HOME/lmr/modules` / `~/.local/share/lmr/modules` (Linux/macOS). También puedes definir `LMR_MODULE_PATH` con una o más carpetas adicionales, separadas por `;` en Windows o `:` en Linux/macOS; cada carpeta puede contener los archivos `.lmr` directamente o dentro de `modules/`. Los imports del proyecto actual tienen prioridad sobre estas rutas.

```powershell
$env:LMR_MODULE_PATH = "D:\lmr-modules"
```

```sh
export LMR_MODULE_PATH="$HOME/lmr-modules"
```

## Límites conocidos de esta versión

La distribución independiente aún está en preparación. Para crear el ZIP portable preliminar de Windows desde un checkout, usa `powershell -ExecutionPolicy Bypass -File .\scripts\package-windows.ps1`; el archivo generado contiene el compilador, sus módulos y el puente nativo. El ZIP todavía requiere que Clang esté instalado en el equipo y disponible por `PATH`. En el checkout de desarrollo, el runtime nativo se localiza desde el repositorio. En los paquetes portables, se encuentra junto al ejecutable. Los módulos se resuelven junto al programa y luego desde la distribución. Por ello, `cargo install` todavía no es una vía de instalación soportada para usuarios finales.

Algunos módulos dependen de funciones nativas, bibliotecas del sistema o una plataforma concreta. Consulta cada entrada del manifiesto y no interpretes el campo de plataforma como una matriz de compatibilidad ya certificada.

## Versionado

El proyecto usa SemVer como objetivo de versionado. Mientras la versión sea `0.x`, una actualización puede cambiar APIs sin las garantías de estabilidad de una versión mayor. La política inicial está en [`VERSIONING.md`](VERSIONING.md).

## Contribuciones

Las propuestas deben seguir [`CONTRIBUTING.md`](CONTRIBUTING.md); los módulos también deben cumplir [`docs/MODULE_API.md`](docs/MODULE_API.md), incluir un ejemplo y declarar requisitos nativos. La licencia del código es MIT; revisa [`LICENSE`](LICENSE).
