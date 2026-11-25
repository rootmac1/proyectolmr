# Identidad visual de LMR

- `lmr-logo.svg`: logo horizontal para documentación y pantallas de bienvenida.
- `lmr-mark.svg`: emblema vectorial compacto.
- `lmr-mark.png`: emblema rasterizado de 512 × 512 para vistas previas.
- `lmr.ico`: icono de Windows incrustado en `lmr.exe`.

## Icono del compilador y de los archivos `.lmr` en Windows

El icono del compilador se incrusta al construir el proyecto con Rust y LLVM:

```powershell
cargo build --release
```

Para mostrar el icono en el Explorador de Windows y añadir una acción explícita al menú contextual, ejecuta:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install-lmr-file-association.ps1 -CompilerPath .\target\release\lmr.exe
```

El script no cambia la aplicación predeterminada que abre los fuentes `.lmr`. Añade **Compilar y ejecutar con LMR** al menú contextual. No requiere privilegios de administrador.

Para desinstalar la asociación y restaurar la asociación anterior, ejecuta:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\uninstall-lmr-file-association.ps1
```
