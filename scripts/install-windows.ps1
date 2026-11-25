param(
    [string]$InstallRoot = (Join-Path $env:LOCALAPPDATA "Programs\LMR"),
    [switch]$NoPath,
    [ValidateSet("User", "Process")]
    [string]$PathScope = "User"
)

$ErrorActionPreference = "Stop"

function Get-PathEntriesWithoutLmrVersions {
    param([AllowNull()][string]$PathValue, [string]$InstallBase)

    $root = [System.IO.Path]::GetFullPath($InstallBase).TrimEnd([char[]]@('\', '/'))
    foreach ($entry in ($PathValue -split ";")) {
        if ([string]::IsNullOrWhiteSpace($entry)) { continue }
        try {
            $fullEntry = [System.IO.Path]::GetFullPath($entry.Trim().Trim('"')).TrimEnd([char[]]@('\', '/'))
        } catch {
            $entry
            continue
        }
        $parent = [System.IO.Path]::GetDirectoryName($fullEntry)
        $leaf = [System.IO.Path]::GetFileName($fullEntry)
        $isLmrVersion = $leaf -match '^\d+\.\d+\.\d+([-.+][0-9A-Za-z.-]+)?$'
        if ($isLmrVersion -and [string]::Equals($parent, $root, [System.StringComparison]::OrdinalIgnoreCase)) {
            continue
        }
        $entry
    }
}

$packageRoot = (Resolve-Path -LiteralPath $PSScriptRoot).Path
$versionFile = Join-Path $packageRoot "VERSION"
if (-not (Test-Path -LiteralPath $versionFile -PathType Leaf)) {
    throw "No se encontró VERSION. Ejecuta este instalador desde el ZIP extraído de LMR."
}
$version = (Get-Content -LiteralPath $versionFile -Raw).Trim()
if ($version -notmatch '^\d+\.\d+\.\d+([-.+][0-9A-Za-z.-]+)?$') {
    throw "El archivo VERSION no contiene una versión válida: $version"
}
$installBase = [System.IO.Path]::GetFullPath($InstallRoot)
$installPath = Join-Path $installBase $version
if (Test-Path -LiteralPath $installPath) {
    throw "Esta versión ya existe en $installPath. Elimina o renombra esa carpeta antes de reinstalar."
}

New-Item -ItemType Directory -Path $installBase -Force | Out-Null
New-Item -ItemType Directory -Path $installPath | Out-Null
try {
    Get-ChildItem -LiteralPath $packageRoot -Force |
        Where-Object { $_.Name -ne "install-windows.ps1" } |
        ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $installPath -Recurse -Force }

    if (-not $NoPath) {
        $currentPath = [Environment]::GetEnvironmentVariable("Path", $PathScope)
        $pathEntries = @(Get-PathEntriesWithoutLmrVersions -PathValue $currentPath -InstallBase $installBase)
        $pathEntries = @($installPath) + $pathEntries
        [Environment]::SetEnvironmentVariable("Path", ($pathEntries -join ";"), $PathScope)

        $processEntries = @(Get-PathEntriesWithoutLmrVersions -PathValue $env:Path -InstallBase $installBase)
        $env:Path = (@($installPath) + $processEntries) -join ";"
    }
} catch {
    if (Test-Path -LiteralPath $installPath) {
        $resolvedBase = (Resolve-Path -LiteralPath $installBase).Path.TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
        $resolvedInstall = (Resolve-Path -LiteralPath $installPath).Path
        if (-not $resolvedInstall.StartsWith($resolvedBase, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Se rechazó retirar una ruta fuera del directorio de instalación: $resolvedInstall"
        }
        Remove-Item -LiteralPath $resolvedInstall -Recurse -Force
    }
    throw
}

Write-Output "LMR $version instalado en $installPath"
if ($NoPath) {
    Write-Output "No se modificó PATH. Ejecuta el binario con la ruta completa: $(Join-Path $installPath 'lmr.exe')"
} elseif ($PathScope -eq "Process") {
    Write-Output "Se puso esta versión al inicio del PATH del proceso actual. Ejecuta lmr en esta sesión de PowerShell."
} else {
    Write-Output "Se puso esta versión al inicio del PATH del usuario. Abre una terminal nueva y ejecuta: lmr --version"
}
