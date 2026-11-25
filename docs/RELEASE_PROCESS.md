# Publicar una versión de LMR

LMR publica paquetes preliminares desde etiquetas de Git. El workflow [`release.yml`](../.github/workflows/release.yml) compila en Windows, Linux y macOS, ejecuta las validaciones y smoke tests, empaqueta la extensión de lenguaje y el tema de iconos del editor como VSIX separados con [`package-extensions.sh`](../scripts/package-extensions.sh), y solo después crea el release con los paquetes y `SHA256SUMS`. La CI también empaqueta ambos VSIX en cada cambio.

## Preparar una versión

1. Actualiza la versión en `Cargo.toml` y `Cargo.lock`.
2. Actualiza la versión actual en `README.md` y `VERSIONING.md`.
3. Añade arriba de `CHANGELOG.md` una sección cuyo encabezado comience con `## <versión> —`; ese bloque se usa como notas del release.
4. Ejecuta las comprobaciones de [`CONTRIBUTING.md`](../CONTRIBUTING.md) y confirma que el paquete portable de Windows pasa su smoke test.
5. Envía los cambios a la rama predeterminada del repositorio.

## Lanzar

Crea una etiqueta que coincida exactamente con Cargo, con `v` al inicio. Por ejemplo, para `0.1.0-alpha.10`:

```sh
git tag -a v0.1.0-alpha.10 -m "LMR 0.1.0-alpha.10"
git push origin v0.1.0-alpha.10
```

El validador `node scripts/validate-release-metadata.js` comprueba que Cargo, README, política de versiones y changelog estén sincronizados. El workflow rechazará la etiqueta si no coincide con la versión de `Cargo.toml` o si las pruebas y el empaquetado de alguna plataforma fallan. Las etiquetas con guion, como `v0.1.0-alpha.10`, producen un prerelease; las versiones estables producen un release normal.

## Verificar descargas

Cada release adjunta archivos `.zip` o `.tar.gz` por plataforma, `lmr-language-support-<versión>.vsix`, `lmr-file-icons-<versión>.vsix` y `SHA256SUMS`. Instala la extensión de lenguaje y, opcionalmente, el tema de iconos en VS Code o Antigravity con **Extensions: Install from VSIX...**. En Linux/macOS se puede comprobar con `sha256sum -c SHA256SUMS`. En Windows, compara el resultado de `Get-FileHash <archivo> -Algorithm SHA256` con la línea correspondiente del archivo de sumas.

La suma detecta descargas dañadas; no es una firma criptográfica del publicador. Los paquetes siguen siendo preliminares hasta que se validen en runners remotos y se cierren las limitaciones anotadas en la hoja de ruta.
