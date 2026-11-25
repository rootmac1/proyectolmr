# Ejemplo de paquete comunitario LMR

`Greeter` es un módulo escrito únicamente en LMR. Expone `greet(string)` y no requiere puentes nativos ni dependencias adicionales. El contrato del paquete está en `lmr-module.json`; la fuente declarada es `Greeter.lmr`.

## Validar y probar

Desde la raíz del repositorio LMR:

```sh
node scripts/validate-module-package.js examples/community
cargo run -- check examples/community/demo.lmr
```

Para consumir el módulo desde otro proyecto, copia `Greeter.lmr` a la carpeta `modules/` de ese proyecto, a la carpeta personal de módulos o a una raíz indicada por `LMR_MODULE_PATH`. Consulta la guía para conocer las rutas de usuario de cada sistema operativo.

## Licencia

Este ejemplo se distribuye bajo MIT. El texto completo está en `LICENSE`.
