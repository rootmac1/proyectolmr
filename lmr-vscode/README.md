# LMR Language Support

Extensión para archivos `.lmr`: resaltado de sintaxis, comentarios, indentación, formato de bloques, snippets, sugerencias de palabras clave/API y diagnósticos del compilador mientras se edita.

## Empaquetar e instalar

Para empaquetar requiere Node.js 22 o posterior y Visual Studio Code o un editor compatible con extensiones VS Code.

```powershell
cd lmr-vscode
npx --yes @vscode/vsce@4.0.0 package --no-dependencies --allow-missing-repository
```

El comando genera un `.vsix`. En VS Code se instala con **Extensions: Install from VSIX...** o desde una terminal con:

```powershell
code --install-extension .\lmr-language-support-0.3.0.vsix
```

El workflow de releases también adjunta este `.vsix` junto con los paquetes del compilador para que pueda instalarse sin compilar la extensión localmente.

Configura `lmr.compilerPath` con la ruta a `lmr.exe` si el compilador no está en `PATH`. La extensión invoca `lmr check --stdin <archivo>` sin escribir archivos temporales; requiere una versión del compilador que incluya el subcomando `check`.

El autocompletado usa palabras clave, funciones incorporadas y las exportaciones del catálogo empacado. Los diagnósticos ejecutan el analizador sintáctico y semántico del compilador con el contenido actual del editor. El formateador ajusta la indentación por bloques `begin/end` y deja intacto el contenido de cadenas y comentarios. La extensión aún no incluye depurador.
