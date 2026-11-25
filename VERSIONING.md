# Versionado y canales de publicación

## Versión actual

El compilador y el repositorio están en `0.1.0-alpha.10`. Es una versión de desarrollo: aún no promete compatibilidad estable de sintaxis, runtime, módulos ni formato de paquetes.

## Esquema previsto

LMR seguirá SemVer cuando se publiquen versiones:

- **MAJOR:** cambios incompatibles en sintaxis, semántica, runtime, CLI o APIs públicas de módulos.
- **MINOR:** funciones nuevas compatibles y módulos/API nuevos.
- **PATCH:** correcciones compatibles.
- **Prelanzamientos:** sufijos como `-alpha.1`, `-beta.1` y `-rc.1` indican versiones previas a una publicación estable.

Mientras `MAJOR` sea cero, la API puede cambiar entre versiones menores. Al anunciar una versión estable, el proyecto debe definir qué superficie se considera pública y durante cuánto tiempo se mantiene la compatibilidad.

## Regla para cambios de módulos

Cada cambio que afecte exportaciones debe actualizar `modules/manifest.json`. Los cambios incompatibles deben documentarse y no se deben presentar como una actualización transparente. Los campos `status` del manifiesto describen madurez estimada del módulo; no sustituyen una política de compatibilidad por versión.

## Publicación

Una publicación etiquetada debe identificar versión del compilador, requisitos de Rust/Clang, plataformas realmente compiladas y limitaciones conocidas. No se etiquetará como estable hasta tener un procedimiento reproducible de instalación y validación por plataforma.
