# LMR: guía de aprendizaje y referencia

**LMR** es un lenguaje compilado, de sintaxis estructurada y tipado estático, cuyo compilador está escrito en Rust. El compilador transforma los programas LMR en LLVM IR y utiliza Clang para generar el ejecutable nativo. Esta guía describe el estado del código fuente de este repositorio; algunas funciones se apoyan en el runtime C y en bibliotecas disponibles en el sistema.

> LMR evoluciona junto con su compilador. Si una construcción descrita aquí falla en tu versión, comprueba primero que estás ejecutando el compilador de este repositorio y consulta los ejemplos incluidos.

## 1. Filosofía y modelo mental

- **Legibilidad estructurada:** bloques explícitos `begin` y `end`; el programa principal termina en `end.`.
- **Tipos declarados:** se escriben los tipos de variables, campos y parámetros. Las funciones que producen un valor declaran su tipo tras `:`.
- **Compilación nativa:** LMR no es Python ni un intérprete general; el compilador genera LLVM IR y delega el enlace final en Clang.
- **Baterías pequeñas y módulos:** consola, matemáticas, gráficos, SQLite y otras integraciones se exponen como módulos `.lmr`, a veces sobre funciones externas del puente nativo.
- **Orientación práctica:** el lenguaje reúne programación procedural, colecciones y clases, más interoperabilidad con C/runtime.

## 2. Primer programa

```lmr
begin
    string nombre
    nombre = "Ada"
    writeln("Hola, " + nombre + "!")
end.
```

Guárdalo como `hola.lmr`. Desde la carpeta del proyecto:

```powershell
cargo run -- run .\hola.lmr
```

El ejecutable acepta también el archivo directamente:

```text
lmr run hola.lmr
lmr hola.lmr
lmr hola.lmr -o hola.exe
lmr hola.lmr -c
lmr check hola.lmr
lmr check --stdin hola.lmr
lmr --help
```

`run` compila y ejecuta. Sin `run`, compila. `check` valida léxico, sintaxis, tipos e imports sin generar ejecutable ni invocar Clang; con `--stdin`, toma el contenido fuente de stdin y usa el archivo indicado para resolver imports. `-o` elige la salida; `-c` genera un objeto LLVM sin enlazar el puente nativo, que debe añadirse al enlazado final si el programa usa el runtime LMR. Sin argumentos, el compilador abre un REPL sencillo, cuyo soporte es limitado frente a un archivo completo. Se requiere Clang accesible desde PATH para compilar programas. En Windows el enlace agrega bibliotecas del sistema; el código de gráficos usa las APIs de Windows.

## 3. Estructura del archivo

El orden general es: importaciones opcionales, declaraciones globales (enum, record, extern, class y func) y, al final, un único bloque principal:

```lmr
from Math import squareRoot

func doble(int n): int
begin
    return n * 2
end

begin
    int resultado
    resultado = doble(21)
    writeln(resultado)
end.
```

No se usan `;`. Los saltos de línea separan sentencias en los lugares admitidos por el lexer; evita poner varias sentencias en una línea. Comentarios: `// hasta el fin de línea` y `/* comentario de varias líneas */`. Las palabras clave se escriben en minúsculas; por convención, los tipos incorporados también (`int`, `float`, etc.).

## 4. Tipos

| Tipo | Uso | Ejemplo |
|---|---|---|
| `int` | Entero con signo; se representa internamente como entero de 64 bits. | `int edad` |
| `float` | Número de punto flotante; internamente doble precisión. | `float precio` |
| `bool` | Lógico: `true` o `false`. | `bool activo` |
| `string` | Cadena de texto. | `string nombre` |
| `char` | Carácter. | `char inicial` |
| `array[T]` | Arreglo homogéneo de `T`. | `array[int] notas` |
| `map[K, V]` | Mapa de claves `K` a valores `V`. | `map[string, int] stock` |
| Tipo declarado | Record o clase definidos en el archivo o un módulo. | `Persona usuario` |
| `ptr` | Puntero usado principalmente en interfaces `extern` y módulos nativos. | `ptr handle` |

Las declaraciones de variables no incluyen inicializador: declara primero y luego asigna. Las asignaciones y argumentos se comprueban semánticamente; no asumas conversión automática entre `int` y `float`. Las constantes usan `const`:

```lmr
const float PI = 3.141592653589793
```

También puede omitirse el tipo para que el compilador lo deduzca del valor, si la expresión tiene un tipo reconocible.

## 5. Expresiones y operadores

| Grupo (de mayor a menor prioridad) | Operadores |
|---|---|
| Unarios | `-`, `not` |
| Multiplicativos | `*`, `/` |
| Aditivos | `+`, `-` |
| Desplazamiento | `shl`, `shr` |
| Comparación | `<`, `>`, `<=`, `>=` |
| Igualdad | `==`, `!=` |
| Bits | `bitand`, `xor`, `bitor` |
| Lógicos | `and`, `or` |

`+` suma números del mismo tipo o concatena cadenas de cadenas. Para incluir un número en texto, conviértelo explícitamente, por ejemplo: `"Puntaje: " + intToString(puntos)`. Comparaciones producen `bool`; `and`, `or` y `not` trabajan con valores booleanos. Usa paréntesis cuando la prioridad no sea obvia.

## 6. Variables, bloques y control de flujo

```lmr
begin
    int contador
    contador = 0

    if contador == 0 then
    begin
        writeln("cero")
    end
    else
    begin
        writeln("distinto de cero")
    end

    while contador < 3 do
    begin
        writeln(contador)
        contador = contador + 1
    end

    for i = 1 to 3 do
    begin
        writeln(i)
    end
end.
```

Iteración sobre colección:

```lmr
array[string] nombres
nombres = ["Ana", "Luis"]
for nombre in nombres do
begin
    writeln(nombre)
end
```

`if` exige `then` y bloque. `while` y ambos `for` exigen `do` y bloque. El `for ... to ...` está destinado a intervalos enteros; revisa ejemplos si dependes del límite final. Los bloques delimitados crean ámbitos para variables locales.

## 7. Funciones y valores de retorno

```lmr
func esPar(int n): bool
begin
    return n % 2 == 0
end
```

La función sin `: tipo` no devuelve un valor y puede terminar sin `return`:

```lmr
func saludar(string nombre)
begin
    writeln("Hola, " + nombre)
end
```

Las funciones con retorno deben devolver un valor en todos los caminos, según el análisis estático. Los parámetros se escriben `Tipo nombre`, separados por coma. Las llamadas usan `nombre(argumentos)`.

## 8. Arreglos y mapas

Los arreglos son homogéneos y usan índices desde cero:

```lmr
array[int] valores
valores = [10, 20, 30]
valores[1] = 25
writeln(valores[1])
writeln(len(valores))
```

También se pueden reservar con tamaño: `valores = new array[int](3)`. El tamaño y los índices se verifican en el runtime.

Los mapas se declaran con ambos tipos y se crean explícitamente:

```lmr
map[string, int] inventario
inventario = new map[string, int]()
inventario["manzanas"] = 5
if contains(inventario, "manzanas") then
begin
    writeln(inventario["manzanas"])
end
```

`len` acepta `string`, `array` o `map`. Consultar una clave inexistente puede producir un error del runtime; compruébala con `contains` antes.

## 9. Records, enums y clases

Los records agrupan campos tipados:

```lmr
record Punto
begin
    float x
    float y
end
```

Las clases admiten campos, métodos, constructor convencional `constructor` y creación con `new`:

```lmr
class Contador
begin
    int valor
    func constructor(int inicial)
    begin
        valor = inicial
    end
    func incrementar()
    begin
        valor = valor + 1
    end
end

begin
    Contador c
    c = new Contador(0)
    c.incrementar()
    writeln(c.valor)
end.
```

Se admite herencia con `extends` y sobrescritura explícita con `override`; la firma del método debe coincidir con la heredada. Se genera despacho virtual para los métodos sobrescritos. La sintaxis declara `enum Nombre begin A, B end`; debido a que su soporte en expresiones/código generado es más limitado que el de clases y records, comprueba su caso de uso con el compilador antes de basar una aplicación en enums.

## 10. Errores, recursos y concurrencia

La sintaxis de excepciones es `try begin ... end catch (error) begin ... end`, y `throw(expresion)` lanza un error. Este mecanismo usa estado de error del runtime; no es un sistema de excepciones Rust y su cobertura depende de qué operaciones reporten el error.

`dispose(variable)` permite liberar explícitamente ciertos objetos administrados. Las clases pueden definir `destructor()` y el compilador registra finalizadores para limpiar objetos al salir de su ámbito. En código que también llama `dispose`, evita usar el objeto después de liberarlo. Para SQLite y otros handles, sigue la API del módulo.

La forma de concurrencia disponible es `spawn nombreFuncion()`. El parser actual no acepta argumentos después del nombre de función en esa sentencia; úsala solo con funciones compatibles y verifica la implementación/runtime antes de depender de sincronización o compartir estado.

## 11. Funciones incorporadas

| Función | Propósito |
|---|---|
| `write(...)`, `writeln(...)` | Escribe uno o varios valores; `writeln` añade salto de línea. |
| `readln()` | Lee texto de consola. También existe una forma con variable destino numérica o string. |
| `len(x)` | Longitud de cadena, arreglo o mapa. |
| `intToString(x)`, `floatToString(x)` | Convierte número a texto. |
| `toUpper(s)`, `toLower(s)` | Cambia mayúsculas/minúsculas. |
| `SliceString(s, inicio[, fin])` | Extrae parte de texto según índices aceptados por runtime. |
| `posString(s, buscado)` | Busca texto y devuelve posición entera. |
| `readFile(ruta)`, `writeFile(ruta, texto)` | Lee o escribe texto; escritura devuelve `bool`. |
| `sleep(ms)` | Espera milisegundos. |
| `sqrt`, `pow`, `abs` | Raíz, potencia y valor absoluto. |
| `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2` | Trigonometría; ángulos en radianes. |
| `exp`, `log`, `log10`, `floor`, `ceil`, `round`, `fmod` | Exponencial, logaritmos, redondeo y módulo real. |
| `jsonParse(texto)` | Interpreta JSON como `map[string, string]`; la forma del mapa limita estructuras JSON anidadas. |
| `contains(mapa, clave)` | Comprueba si existe una clave. |
| `dispose(objeto)` | Liberación explícita de recurso administrado. |

El módulo `Math` expone sus funciones con nombres en inglés, como `squareRoot`, `power`, `sine`, `degreesToRadians`, `hypotenuse`, `clamp` e `interpolate`; importa solo los símbolos que uses: `from Math import squareRoot, power`. El módulo separado `Matematicas` contiene ejemplos con nombres en español.

## 12. Módulos disponibles

Los imports pueden ser `import Graphics` o `from Math import squareRoot`. El nombre con puntos se convierte en carpetas y nombre de archivo `.lmr`. El compilador busca primero desde la carpeta del programa (en `modules/` y luego como ruta directa), después en las rutas de `LMR_MODULE_PATH`, en la carpeta de módulos del usuario y, por último, en la distribución de LMR.

Para instalar módulos comunitarios manualmente, copia los archivos declarados por el paquete a `%LOCALAPPDATA%\LMR\modules` en Windows o a `$XDG_DATA_HOME/lmr/modules` / `~/.local/share/lmr/modules` en Linux/macOS. También puedes configurar rutas adicionales:

```powershell
$env:LMR_MODULE_PATH = "D:\lmr-modules"
```

```sh
export LMR_MODULE_PATH="$HOME/lmr-modules"
```

Si agregas varias rutas, sepáralas con `;` en Windows o `:` en Linux/macOS. Cada raíz puede contener los `.lmr` directamente o dentro de un subdirectorio `modules/`.

| Módulo | Qué ofrece |
|---|---|
| `Math` / `Matematicas` | Funciones matemáticas en inglés/español según el módulo. |
| `Console` | Colores ANSI y limpieza de consola. |
| `Graphics` | Ventana 2D, dibujo, texto, tabla y texto desplazable. |
| `SQLite` | Clases `Database` y `Cursor` sobre SQLite. |
| `Trade` | Funciones del puente de trading del proyecto. |
| `ReportArgs` | Lectura de parámetros de informe desde el runtime. |
| `Bet365` | Importación de CSV Bet365 hacia SQLite. |

Estos módulos son parte del repositorio, no necesariamente una biblioteca estándar portable. Algunos dependen del puente `src/trade_bridge.c`, sistema operativo, archivos de datos o bibliotecas externas. Por ejemplo, importa `Graphics` y llama sus funciones de ciclo (`initWindow`, `beginDrawing`, `endDrawing`, `closeWindow`) según el ejemplo existente; las firmas están en `modules/Graphics.lmr`.

## 13. Funciones externas e interoperabilidad

`extern func` declara una función que el enlace debe encontrar en código nativo:

```lmr
extern func nombre_nativo(int valor): int
```

El compilador genera la declaración LLVM, pero no implementa esa función automáticamente. Debe existir en el puente o en una biblioteca enlazada; respeta exactamente tipos y firma. El tipo `ptr` se usa para handles C. El módulo `SQLite` es un ejemplo completo.

## 14. Diagnósticos y consejos

- Los errores de léxico, sintaxis y análisis semántico normalmente señalan archivo, línea y columna.
- Declara cada variable antes de utilizarla y no la vuelvas a declarar en el mismo ámbito.
- Verifica que el tipo de asignación, retorno y argumentos concuerde.
- Revisa que cada bloque tenga su `end` y el principal `end.`.
- Importa símbolos con mayúsculas/minúsculas idénticas a las del módulo.
- Si aparece un error LLVM, conserva el `.ll` generado y el mensaje íntegro: puede ser un fallo del backend o una incompatibilidad de firma, distinto de un error de sintaxis.
- Ejecuta primero ejemplos pequeños y después las demos del repositorio (`test_arrays.lmr`, `test_loops.lmr`, `test_oop.lmr`, `test_sqlite.lmr`, `demo_graficos.lmr`).

## 15. Ejemplo completo

```lmr
from Math import hypotenuse

func clasificar(float nota): string
begin
    if nota >= 11.0 then
    begin
        return "aprobado"
    end
    else
    begin
        return "por reforzar"
    end
end

begin
    float x, y, distancia
    string estado
    x = 3.0
    y = 4.0
    distancia = hypotenuse(x, y)
    estado = clasificar(distancia + 7.0)
    writeln("Distancia: " + floatToString(distancia))
    writeln("Resultado: " + estado)
end.
```

## 16. Mapa de implementación

El compilador principal está en `src/`: `lexer.rs` reconoce tokens, `parser.rs` construye la sintaxis, `ast.rs` define el árbol, `sema.rs` valida tipos/ámbitos y `codegen.rs` genera LLVM IR. El ejecutable y la integración de módulos están coordinados desde `main.rs`; el runtime nativo está principalmente en `trade_bridge.c`. Esta división ayuda a localizar un problema: errores antes de LLVM suelen ser de código fuente, mientras que errores de enlazado o ejecución pueden pertenecer a la interfaz nativa.
