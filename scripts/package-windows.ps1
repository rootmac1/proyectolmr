param(
    [string]$Target = "x86_64-pc-windows-msvc",
    [string]$OutputDirectory
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$cargoToml = Get-Content (Join-Path $repoRoot "Cargo.toml") -Raw
$versionMatch = [regex]::Match($cargoToml, '(?m)^version\s*=\s*"([^"]+)"')
if (-not $versionMatch.Success) {
    throw "No se pudo leer la versión de Cargo.toml"
}
$targetMatch = [regex]::Match($Target, '^(?<arch>[A-Za-z0-9_]+)-pc-windows-(msvc|gnu)$')
if (-not $targetMatch.Success) {
    throw "Target Windows no válido: $Target. Usa un target Rust con formato <arquitectura>-pc-windows-msvc|gnu."
}
$version = $versionMatch.Groups[1].Value
$architecture = $targetMatch.Groups['arch'].Value
$targetDir = Join-Path $repoRoot "target\$Target\release"
$exePath = Join-Path $targetDir "lmr.exe"
$distDir = if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    Join-Path $repoRoot "dist"
} else {
    [System.IO.Path]::GetFullPath($OutputDirectory)
}
$packageName = "lmr-$version-windows-$architecture"
$stageDir = Join-Path $distDir $packageName
$zipPath = Join-Path $distDir "$packageName.zip"

if (Test-Path -LiteralPath $stageDir) {
    throw "La carpeta de salida ya existe; muévela manualmente antes de empaquetar: $stageDir"
}
if (Test-Path -LiteralPath $zipPath) {
    throw "El ZIP de salida ya existe; muévelo manualmente antes de empaquetar: $zipPath"
}

Push-Location $repoRoot
try {
    & cargo build --locked --release --target $Target
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build terminó con código $LASTEXITCODE"
    }
} finally {
    Pop-Location
}

if (-not (Test-Path -LiteralPath $exePath -PathType Leaf)) {
    throw "No se encontró el compilador generado: $exePath"
}

New-Item -ItemType Directory -Path $distDir -Force | Out-Null
New-Item -ItemType Directory -Path $stageDir | Out-Null
New-Item -ItemType Directory -Path (Join-Path $stageDir "src") | Out-Null
New-Item -ItemType Directory -Path (Join-Path $stageDir "docs") | Out-Null
New-Item -ItemType Directory -Path (Join-Path $stageDir "modules") | Out-Null
New-Item -ItemType Directory -Path (Join-Path $stageDir "examples\community") | Out-Null
New-Item -ItemType Directory -Path (Join-Path $stageDir "scripts") | Out-Null
Copy-Item -LiteralPath $exePath -Destination (Join-Path $stageDir "lmr.exe")
$moduleSource = Join-Path $repoRoot "modules"
Get-ChildItem -LiteralPath $moduleSource -File |
    Where-Object { $_.Name -ne "TEMPLATE.lmr" } |
    ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $stageDir "modules") }
Copy-Item -LiteralPath (Join-Path $repoRoot "src\trade_bridge.c") -Destination (Join-Path $stageDir "src\trade_bridge.c")
Copy-Item -LiteralPath (Join-Path $repoRoot "README.md"), (Join-Path $repoRoot "CONTRIBUTING.md"), (Join-Path $repoRoot "LICENSE"), (Join-Path $repoRoot "VERSIONING.md"), (Join-Path $repoRoot "CHANGELOG.md"), (Join-Path $repoRoot "GUIA_LMR_ES.md") -Destination $stageDir
Copy-Item -LiteralPath (Join-Path $repoRoot "docs\MODULE_API.md") -Destination (Join-Path $stageDir "docs\MODULE_API.md")
Copy-Item -LiteralPath (Join-Path $repoRoot "docs\module-package.schema.json") -Destination (Join-Path $stageDir "docs\module-package.schema.json")
Get-ChildItem -LiteralPath (Join-Path $repoRoot "examples\community") -File |
    ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $stageDir "examples\community") }
Copy-Item -LiteralPath (Join-Path $repoRoot "scripts\validate-module-package.js") -Destination (Join-Path $stageDir "scripts\validate-module-package.js")
Copy-Item -LiteralPath (Join-Path $repoRoot "scripts\install-windows.ps1"), (Join-Path $repoRoot "scripts\uninstall-windows.ps1") -Destination $stageDir
[System.IO.File]::WriteAllText((Join-Path $stageDir "VERSION"), $version, [System.Text.UTF8Encoding]::new($false))

Compress-Archive -Path (Join-Path $stageDir "*") -DestinationPath $zipPath
Write-Output "Paquete portable creado: $zipPath"
Write-Output "Requiere Clang en PATH para compilar programas .lmr."
