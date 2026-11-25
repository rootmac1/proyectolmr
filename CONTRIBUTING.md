# Contribuir a LMR

Gracias por ayudar a mejorar LMR. El proyecto está en alpha y algunas interfaces pueden cambiar; describe cualquier cambio incompatible en el pull request y actualiza la documentación correspondiente.

## Preparar el entorno

- Rust 1.85 o posterior (Edition 2024; también declarado en `Cargo.toml`).
- Clang en `PATH` para enlazar y ejecutar programas LMR.
- Node.js 20 o posterior para las pruebas de la extensión; Node.js 22 o posterior para empaquetar los VSIX.
- PowerShell 7 para empaquetar/validar en Windows; Bash para empaquetar/validar en Linux y macOS.

## Comprobaciones antes de enviar cambios

Desde la raíz del repositorio:

```text
cargo fmt --all -- --check
cargo test --locked
cargo run --locked -- check demo.lmr
node --test scripts/validate-crate-package.test.js
node scripts/validate-crate-package.js
cargo package --locked --allow-dirty
node --test lmr-vscode/test/formatter.test.js
node --test scripts/validate-module-package.test.js
node --test scripts/validate-release-metadata.test.js
node scripts/validate-release-metadata.js
node --test scripts/extract-release-notes.test.js
node scripts/validate-module-package.js examples/community
cargo run -- check examples/community/demo.lmr
```

Si modificas un módulo, valida el catálogo y su espejo empacado para el editor:

```powershell
.\scripts\validate-module-manifest.ps1
```

Si modificas el compilador, los módulos o instaladores, ejecuta el paquete y el smoke test de la plataforma disponible:

```powershell
.\scripts\package-windows.ps1
.\scripts\smoke-portable-windows.ps1 -Archive .\dist\lmr-<version>-windows-x86_64.zip
```

```sh
bash scripts/package-unix.sh
bash scripts/smoke-portable-unix.sh dist/lmr-<version>-<os>-<arch>.tar.gz
```

El CI ejecuta formato, pruebas Rust, validación del contenido del crate, compilación del paquete, validación del catálogo, prueba del editor y empaquetado portable en la matriz configurada. Declara únicamente plataformas que hayas probado.

## Añadir un módulo LMR

Sigue [`docs/MODULE_API.md`](docs/MODULE_API.md). Para un módulo incluido en LMR, actualiza `modules/manifest.json` y su copia `lmr-vscode/modules/manifest.json`. Para un paquete comunitario, usa `lmr-module.json` como en `examples/community/`. Valida el contrato con `node scripts/validate-module-package.js <directorio>` y revisa el programa consumidor con `lmr check`; esto valida los metadatos, pero no instala el paquete ni resuelve dependencias automáticamente.

Si el módulo usa `extern`, incluye el puente nativo, manejo de errores/recursos y las bibliotecas requeridas. No anuncies como disponible una función externa que no enlace en la plataforma declarada.

## Cambios al compilador o runtime

Actualiza lexer/parser/AST/análisis semántico/backend según corresponda. Incluye una prueba de regresión para el comportamiento nuevo y verifica los errores de compilación para que terminen con un código de salida distinto de cero. Evita cambios de sintaxis incompatibles salvo que exista una razón clara y documentada.

## Seguridad y datos

No incluyas claves, cookies, tokens, bases de datos personales ni ejecutables generados. Las funciones nativas y módulos LMR se ejecutan con los permisos del proceso que los usa; documenta efectos de red, archivos y sistema.

## Licencia

Las contribuciones se distribuyen bajo MIT junto con el proyecto. No aportes código de terceros sin confirmar que su licencia es compatible.
