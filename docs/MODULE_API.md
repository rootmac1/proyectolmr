# API de módulos de LMR (propuesta v1)

Este documento fija un contrato práctico para describir, consumir y aportar módulos de LMR. El inventario oficial está en [`../modules/manifest.json`](../modules/manifest.json), en JSON para que lo lean tanto las personas como editores, generadores de documentación y herramientas de comunidad.

## Alcance de la v1

En esta versión, un módulo es un archivo `.lmr` cargado por el compilador desde `modules/`, una ruta configurada en `LMR_MODULE_PATH`, la carpeta de módulos del usuario o la ruta relativa del programa. No es una biblioteca dinámica: el cargador combina las declaraciones del archivo con el programa y el backend enlaza las funciones externas declaradas. El manifiesto del paquete documenta los archivos y sus exportaciones; el compilador aún no lo consulta para instalar módulos ni resolver dependencias automáticamente.

Una extensión puede ser:

1. **Pura LMR:** funciones, records y clases implementados con la sintaxis del lenguaje.
2. **Con puente nativo:** declaraciones `extern func` en el `.lmr`, acompañadas por una implementación ABI compatible en el runtime/puente nativo y su configuración de enlace.

## Importar un módulo

```lmr
import Graphics
from Math import raiz, potencia

begin
    float x
    x = raiz(potencia(3.0, 2.0) + potencia(4.0, 2.0))
    writeln(x)
end.
```

El nombre del módulo es el nombre del archivo sin `.lmr`; los puntos se convierten en separadores de carpeta. `from M import a, b` comprueba que los símbolos puedan resolverse al compilar. Los módulos pueden importar otros módulos; el cargador evita procesar dos veces un mismo nombre durante una compilación.

Para analizar un programa sin generar LLVM ni llamar a Clang, usa `lmr check programa.lmr`. La forma `lmr check --stdin programa.lmr` lee el contenido fuente desde stdin y conserva la ruta indicada para resolver imports relativos; esta forma la usa la extensión de VS Code para revisar cambios sin guardar archivos temporales.

## Formato del manifiesto

`modules/manifest.json` usa `schema_version` para versionar el formato. Cada entrada declara:

- `id`, `file`, `summary` y `category` para identificar y descubrir el módulo.
- `status`: `stable`, `experimental` o `internal`; no implica garantías de compatibilidad más allá de lo escrito en la entrada.
- `platforms`: sistemas en los que se espera que funcione; `all` significa que no tiene una restricción conocida del módulo, no que todas las dependencias hayan sido probadas.
- `dependencies`: otros módulos o requisitos del runtime/sistema.
- `exports`: API visible (funciones, clases, records o constantes convencionales) y sus firmas en LMR.
- `native_bindings`: símbolos `extern` que deben resolverse al enlazar, si los hay.

No edites el manifiesto a mano sin revisar también el archivo `.lmr`: debe coincidir con lo que el compilador realmente carga. Los campos de descripción son metadatos, no alteran compilación.

## Contrato de una exportación

- Usa tipos LMR explícitos en parámetros y valores de retorno.
- Documenta unidades, índices, rangos válidos, errores y propiedad/liberación de recursos.
- Devuelve resultados mediante el tipo declarado; si una función falla, documenta si devuelve indicador/valor centinela o si lanza error.
- No guardes estado global implícito si el módulo puede ofrecer una API explícita de inicialización y cierre.
- No expongas `ptr` a quien usa el módulo salvo que forme parte de un contrato nativo documentado.
- Mantén los nombres existentes y semántica compatible; si se rompe una API, registra el cambio y aumenta la versión del módulo en el manifiesto.

## Módulos con funciones `extern`

Cada símbolo externo debe tener una sola declaración con firma exacta. Para contribuciones nativas, incluye:

1. La declaración `extern func` en el módulo LMR.
2. Implementación en el puente o biblioteca, con convención de llamada y representación de cada tipo compatibles con el backend actual.
3. Cambios mínimos necesarios en configuración de compilación/enlace por plataforma.
4. Manejo de error y ciclo de vida de handles descritos en documentación.
5. Pruebas de compilación y ejecución para la plataforma soportada.

No asumas que el enlazador encontrará una función solo porque está declarada en LMR. Si la implementación no está enlazada, el ejecutable puede fallar al enlazar.

El puente de red de `Trade` invoca `curl`. Para evitar que una URL se interprete como sintaxis del shell, el puente acepta únicamente URLs ASCII en minúsculas con esquema `http`/`https` para HTTP y `ws`/`wss` para WebSocket; rechaza credenciales en la autoridad, caracteres de control, comillas, expansiones de shell, `%` y URLs de más de 1800 caracteres. Usa rutas y parámetros ASCII sin codificar porcentajes. Las llamadas HTTP limitan los esquemas de redirección a HTTP(S); no se debe pasar una URL recibida de una fuente no confiable sin validarla.

## Crear un módulo comunitario

1. Crea `modules/Nombre.lmr` usando `modules/TEMPLATE.lmr` como referencia.
2. Mantén el módulo autocontenido; importa dependencias explícitamente.
3. Añade una entrada a `modules/manifest.json`, con firmas verificadas contra el archivo.
4. Añade un ejemplo mínimo de uso bajo `examples/` (si aún no existe, crea esa carpeta) y documenta requisitos externos.
5. Compila y ejecuta el ejemplo con el compilador de este repositorio. Para módulos nativos, prueba cada plataforma que declares.
6. Envía el cambio incluyendo fuentes, documentación y pasos de validación; nunca incluyas secretos, tokens, bases de datos personales o binarios generados.

## Lista de revisión para aportes

- [ ] El nombre del archivo y `id` del manifiesto coinciden.
- [ ] Cada exportación y firma coincide con el `.lmr`.
- [ ] Imports y dependencias están declarados.
- [ ] Se documentan plataformas, efectos y errores.
- [ ] La API libera/cierra sus recursos y explica quién es responsable.
- [ ] El ejemplo demuestra el camino habitual de uso.
- [ ] Se validó compilación/ejecución; no se afirman plataformas no probadas.
- [ ] No hay cambios incompatibles ocultos.

## Evolución hacia paquetes instalables

El catálogo oficial `modules/manifest.json` describe los módulos incluidos en LMR. Para compartir módulos comunitarios sin copiarlos al núcleo se define además `lmr-module.json`; su esquema está en [`module-package.schema.json`](module-package.schema.json) y el ejemplo completo en [`../examples/community/`](../examples/community/). Valídalo con `node scripts/validate-module-package.js <directorio>`. Para usarlo, copia los archivos `.lmr` declarados a la carpeta de módulos del usuario o a una ruta de `LMR_MODULE_PATH`, y prueba el programa consumidor con `lmr check <archivo.lmr>`. Las dependencias del manifiesto se deben instalar manualmente; todavía no hay un registro ni un gestor de paquetes.

Este contrato permite revisar metadatos y fuentes, pero no instala paquetes ni descarga dependencias. La v1 todavía no resuelve dependencias/versiones, no ofrece repositorio, hashes/firma, aislamiento de código nativo ni ABI estable. Esos puntos deben acordarse antes de implementar un gestor de paquetes; el esquema versionado permite evolucionar el contrato sin presentarlo como una ABI estable.
