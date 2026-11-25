# Registro de cambios

Los cambios de cada versión se resumen aquí. LMR sigue en fase alpha; el registro no implica compatibilidad estable.

## 0.1.0-alpha.10 — validación segura de URLs

- Rechaza URLs con expansiones de shell, credenciales incrustadas, esquemas distintos de HTTP(S)/WS(S), controles o longitud excesiva antes de invocar `curl`.
- Añade una prueba nativa de regresión para URLs seguras y casos de inyección.
- Permite que el instalador de Windows modifique solo el PATH del proceso para pruebas y automatización; el smoke test comprueba ese modo sin alterar el perfil de usuario.
- Añade empaquetado VSIX para la extensión de lenguaje y el tema de iconos, con extracción de notas de versión en release.
- Declara Rust 1.85 como versión mínima requerida por Edition 2024.
- Hace que el empaquetador de Windows valide el target y etiquete el paquete con su arquitectura.
- Añade desinstaladores para Windows y Unix que retiran solo la versión indicada y su entrada de PATH/lanzador correspondiente.
- Corrige las actualizaciones de Windows/Unix para seleccionar la nueva versión en PATH o en el lanzador sin borrar instalaciones anteriores ni reemplazar comandos ajenos.
- Permite resolver módulos comunitarios desde `LMR_MODULE_PATH` y una carpeta de módulos por usuario.
- Alinea el validador comunitario con el esquema al comprobar URI de repositorio y plataformas duplicadas, y endurece la contención de rutas.
- Comparte un único script de empaquetado VSIX entre CI y release para validar las extensiones antes de publicar.
- Añade formularios de GitHub para reportar errores y proponer mejoras, y una plantilla de pull request.
- Limita el crate de Cargo a fuentes, módulos, documentación y ejemplos; CI verifica su contenido para excluir bases de datos, ejecutables y artefactos locales.
- Enlaza el manifiesto Cargo y el README al repositorio oficial `rootmac1/proyectolmr`.
- Excluye del repositorio los binarios/paquetes generados y aplicaciones, datos, cookies y capturas locales.

## 0.1.0-alpha.9 — preparación de distribución

- Define licencia MIT, política inicial de versiones, guía en español y contrato de contribución.
- Añade paquetes portables preliminares para Windows, Linux y macOS, con instaladores y pruebas smoke; el ZIP de Windows se validó localmente.
- Añade `lmr check` para validar programas sin generar ejecutables, y pruebas para el compilador y la extensión del editor.
- Incluye catálogo de módulos, extensión de VS Code y contrato versionado para paquetes comunitarios con esquema, validador y ejemplo.
- El CI está configurado para Windows, Linux y macOS; sus resultados remotos aún están pendientes.
- Los paquetes necesitan Clang para compilar programas; algunos módulos requieren además herramientas o bibliotecas del sistema.
