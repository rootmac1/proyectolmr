[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = "High")]
param(
    [string]$InstallRoot = (Join-Path $env:LOCALAPPDATA "Programs\LMR"),
    [string]$Version,
    [switch]$NoPath,
    [ValidateSet("User", "Process")]
    [string]$PathScope = "User"
)

$ErrorActionPreference = "Stop"
$packageRoot = (Resolve-Path -LiteralPath $PSScriptRoot).Path
if (-not $PSBoundParameters.ContainsKey("Version")) {
    $versionFile = Join-Path $packageRoot "VERSION"
    if (-not (Test-Path -LiteralPath $versionFile -PathType Leaf)) {
        throw "Indica -Version o ejecuta este desinstalador desde un paquete LMR que contenga VERSION."
    }
    $Version = (Get-Content -LiteralPath $versionFile -Raw).Trim()
}
if ($Version -notmatch '^\d+\.\d+\.\d+([-.+][0-9A-Za-z.-]+)?$') {
    throw "La versión no es válida: $Version"
}

$installBase = [System.IO.Path]::GetFullPath($InstallRoot)
if (-not (Test-Path -LiteralPath $installBase -PathType Container)) {
    throw "No se encontró la raíz de instalación: $installBase"
}
$resolvedBase = (Resolve-Path -LiteralPath $installBase).Path.TrimEnd([System.IO.Path]::DirectorySeparatorChar)
$installPrefix = $resolvedBase + [System.IO.Path]::DirectorySeparatorChar
$installPath = [System.IO.Path]::GetFullPath((Join-Path $resolvedBase $Version))
if (-not $installPath.StartsWith($installPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Se rechazó una ruta de instalación fuera de InstallRoot: $installPath"
}
if (-not (Test-Path -LiteralPath $installPath -PathType Container)) {
    throw "No se encontró LMR $Version en $installPath"
}
$resolvedInstall = (Resolve-Path -LiteralPath $installPath).Path
if (-not $resolvedInstall.StartsWith($installPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Se rechazó retirar una ruta fuera de InstallRoot: $resolvedInstall"
}

if ($PSCmdlet.ShouldProcess($resolvedInstall, "Desinstalar LMR $Version")) {
    if (-not $NoPath) {
        $currentPath = [Environment]::GetEnvironmentVariable("Path", $PathScope)
        $remainingEntries = @($currentPath -split ";" | Where-Object {
            $_ -and -not [string]::Equals($_.TrimEnd("\"), $resolvedInstall.TrimEnd("\"), [System.StringComparison]::OrdinalIgnoreCase)
        })
        [Environment]::SetEnvironmentVariable("Path", ($remainingEntries -join ";"), $PathScope)
    }
    if (-not $NoPath) {
        $processEntries = @($env:Path -split ";" | Where-Object {
            $_ -and -not [string]::Equals($_.TrimEnd("\"), $resolvedInstall.TrimEnd("\"), [System.StringComparison]::OrdinalIgnoreCase)
        })
        $env:Path = $processEntries -join ";"
    }

    Remove-Item -LiteralPath $resolvedInstall -Recurse -Force
    Write-Output "LMR $Version desinstalado de $resolvedInstall"
    if ($NoPath) {
        Write-Output "No se modificó PATH."
    } else {
        Write-Output "Se retiró la carpeta de LMR del PATH de $PathScope."
    }
}
