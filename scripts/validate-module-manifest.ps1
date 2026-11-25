$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$modulesRoot = Join-Path $repoRoot "modules"
$manifestPath = Join-Path $modulesRoot "manifest.json"
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$editorManifestPath = Join-Path $repoRoot "lmr-vscode\modules\manifest.json"

if ($manifest.schema_version -notmatch '^\d+\.\d+\.\d+$') {
    throw "schema_version debe usar formato SemVer"
}

$seenIds = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
$declaredFiles = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
foreach ($module in $manifest.modules) {
    if (-not $seenIds.Add($module.id)) {
        throw "ID de módulo duplicado: $($module.id)"
    }
    if ([System.IO.Path]::GetFileName($module.file) -cne $module.file) {
        throw "La ruta del módulo debe ser un nombre de archivo dentro de modules/: $($module.file)"
    }
    if (-not $declaredFiles.Add($module.file)) {
        throw "Archivo de módulo duplicado en el manifiesto: $($module.file)"
    }

    $modulePath = Join-Path $modulesRoot $module.file
    if (-not (Test-Path -LiteralPath $modulePath -PathType Leaf)) {
        throw "No existe el archivo del módulo '$($module.file)'"
    }
    if ([System.IO.Path]::GetFileNameWithoutExtension($module.file) -cne $module.id) {
        throw "El ID '$($module.id)' no coincide con el nombre '$($module.file)'"
    }

    $source = Get-Content -LiteralPath $modulePath -Raw
    foreach ($export in $module.exports) {
        $escapedName = [regex]::Escape($export.name)
        if ($export.kind -ceq "function") {
            $pattern = "(?m)^\s*func\s+$escapedName\s*\("
        } elseif ($export.kind -ceq "class") {
            $pattern = "(?m)^\s*class\s+$escapedName\b"
        } elseif ($export.kind -ceq "record") {
            $pattern = "(?m)^\s*record\s+$escapedName\b"
        } elseif ($export.kind -ceq "enum") {
            $pattern = "(?m)^\s*enum\s+$escapedName\b"
        } else {
            throw "Tipo de exportación desconocido '$($export.kind)' en $($module.id)"
        }
        if ($source -cnotmatch $pattern) {
            throw "No se encontró la exportación '$($export.name)' en $($module.file)"
        }
    }

    foreach ($binding in $module.native_bindings) {
        $escapedName = [regex]::Escape($binding)
        if ($source -cnotmatch "(?m)^\s*extern\s+func\s+$escapedName\s*\(") {
            throw "No se encontró el binding nativo '$binding' en $($module.file)"
        }
    }
}

Get-ChildItem -LiteralPath $modulesRoot -Filter "*.lmr" -File |
    Where-Object { $_.Name -cne "TEMPLATE.lmr" } |
    ForEach-Object {
        if (-not $declaredFiles.Contains($_.Name)) {
            throw "El módulo '$($_.Name)' no está en manifest.json"
        }
    }

if (-not (Test-Path -LiteralPath $editorManifestPath -PathType Leaf)) {
    throw "No se encontró el catálogo empacado para la extensión del editor"
}
$editorManifest = Get-Content -LiteralPath $editorManifestPath -Raw | ConvertFrom-Json
$manifestJson = ConvertTo-Json -InputObject $manifest -Depth 100 -Compress
$editorManifestJson = ConvertTo-Json -InputObject $editorManifest -Depth 100 -Compress
if ($manifestJson -cne $editorManifestJson) {
    throw "El catálogo de lmr-vscode/modules/manifest.json está desactualizado; cópialo desde modules/manifest.json"
}

Write-Output "Manifiesto válido: $($manifest.modules.Count) módulos, exportaciones y copia del editor comprobados."
