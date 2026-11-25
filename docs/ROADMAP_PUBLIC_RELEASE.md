# Hoja de ruta para publicar LMR

Seguimiento del trabajo para pasar del checkout actual a una distribución que la comunidad pueda instalar y mantener.

| Etapa | Estado | Alcance |
|---|---|---|
| 1. Identidad y versión inicial | Preliminar | MIT, `0.1.0-alpha.10`, README, changelog, requisitos mínimos en Cargo y política inicial de versión. El crate enlaza al repositorio `rootmac1/proyectolmr` y tiene una lista blanca de archivos para no incluir datos, binarios ni artefactos locales. |
| 2. Empaquetado portable de Windows | Preliminar validado localmente y en CI | ZIP con compilador, módulos, puente, docs, instalador/desinstalador por usuario, notas de versión y ejemplo/contrato de paquete comunitario; el empaquetador valida el target y usa su arquitectura en el nombre. El smoke test remoto valida ejecución, instalación, actualización del PATH y desinstalación fuera del checkout. Requiere Clang externo. |
| 3. Calidad y CI | Preliminar, matriz remota verde | Pasan 22 pruebas Rust y 19 de Node, incluyendo 4 de inventario del crate; `cargo package --locked --allow-dirty` verifica el crate de 40 archivos y su compilación, sin datos, ejecutables ni `dist/`. El commit `fa67927` pasó los jobs de MSRV 1.85, Windows, Ubuntu, macOS y empaquetado VSIX; los smokes de Windows, Linux y macOS también pasaron. |
| 4. Instalación y portabilidad | Preliminar, smokes remotos verdes | Instaladores/desinstaladores y empaquetadores preliminares para Windows/Linux/macOS. Los smoke tests remotos validan los tres paquetes, incluidas la actualización del lanzador Unix y la protección de comandos ajenos. El módulo gráfico y SQLite siguen limitados a Windows. |
| 5. Herramientas de desarrollo | Preliminar | Extensión VS Code con resaltado, snippets, completado, diagnósticos con `lmr check --stdin` y formato `begin/end`; tema de iconos para `.lmr`. CI y release empaquetan e inspeccionan VSIX separados para ambos. Language server dedicado y depurador quedan pendientes. |
| 6. Módulos comunitarios | Preliminar | Catálogo del núcleo sincronizado con el editor, esquema `lmr-module.json`, validador con pruebas, ejemplo de paquete y búsqueda por carpeta de usuario/`LMR_MODULE_PATH`. Hay formularios de GitHub para reportes y propuestas, además de una plantilla de pull request. La instalación del paquete sigue siendo manual; no hay registro ni resolución automática de dependencias, y las versiones aún no garantizan compatibilidad ABI. |
| 7. Publicación de releases | Preliminar | El repositorio público es `rootmac1/proyectolmr` y la CI de `main` pasó en las tres plataformas. El workflow por etiqueta está configurado para publicar paquetes Windows/Linux/macOS, VSIX, sumas SHA-256 y notas del changelog; falta probar una etiqueta y publicar los primeros assets. |

## Criterio de salida de alpha

LMR no se debe anunciar como distribución estable mientras la instalación dependa del checkout de desarrollo, los módulos/puente no estén empaquetados y validados, o las plataformas declaradas no tengan compilación reproducible. Cada etapa se actualiza con el trabajo y evidencia obtenida; las marcas “preliminar” no equivalen a aprobación de release.
