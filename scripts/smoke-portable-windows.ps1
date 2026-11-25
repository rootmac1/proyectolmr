param(
    [Parameter(Mandatory = $true)]
    [string]$Archive
)

$ErrorActionPreference = "Stop"
$archivePath = (Resolve-Path -LiteralPath $Archive).Path
$tempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
$smokeId = [guid]::NewGuid().ToString("N")
$workDir = Join-Path $tempRoot ("lmr-portable-smoke-" + $smokeId)
$runDir = Join-Path $tempRoot ("lmr-portable-run-" + $smokeId)
$installRoot = Join-Path $tempRoot ("lmr-portable-install-" + $smokeId)
$userInstallRoot = Join-Path $tempRoot ("lmr-portable-user-install-" + $smokeId)
$pathInstallRoot = Join-Path $tempRoot ("lmr-portable-path-install-" + $smokeId)
$originalProcessPath = $env:Path
$originalUserPath = [Environment]::GetEnvironmentVariable("Path", "User")
$userPathChanged = $false
$workDirFull = [System.IO.Path]::GetFullPath($workDir)
foreach ($candidate in @($workDirFull, [System.IO.Path]::GetFullPath($runDir), [System.IO.Path]::GetFullPath($installRoot), [System.IO.Path]::GetFullPath($userInstallRoot), [System.IO.Path]::GetFullPath($pathInstallRoot))) {
    if (-not $candidate.StartsWith($tempRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Directorio temporal fuera de la raíz temporal"
    }
}

New-Item -ItemType Directory -Path $workDirFull | Out-Null
New-Item -ItemType Directory -Path $runDir | Out-Null
try {
    Expand-Archive -LiteralPath $archivePath -DestinationPath $workDirFull

    $communityExample = Join-Path $workDirFull "examples\community\demo.lmr"
    $communityCheck = & (Join-Path $workDirFull "lmr.exe") check $communityExample 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        throw "El ejemplo comunitario no pasó lmr check desde el paquete: $communityCheck"
    }
    if (-not (Test-Path -LiteralPath (Join-Path $workDirFull "docs\module-package.schema.json"))) {
        throw "El paquete no contiene el esquema del contrato comunitario"
    }
    if (-not (Test-Path -LiteralPath (Join-Path $workDirFull "examples\community\LICENSE"))) {
        throw "El paquete no contiene la licencia del ejemplo comunitario"
    }
    if (-not (Test-Path -LiteralPath (Join-Path $workDirFull "examples\community\README.md"))) {
        throw "El paquete no contiene las instrucciones del ejemplo comunitario"
    }
    if (-not (Test-Path -LiteralPath (Join-Path $workDirFull "CHANGELOG.md"))) {
        throw "El paquete no contiene el registro de cambios"
    }

    $moduleRoot = Join-Path $runDir "community-modules"
    $consumerRoot = Join-Path $runDir "community-consumer"
    New-Item -ItemType Directory -Path (Join-Path $moduleRoot "modules") -Force | Out-Null
    New-Item -ItemType Directory -Path $consumerRoot -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $workDirFull "examples\community\Greeter.lmr") -Destination (Join-Path $moduleRoot "modules\Greeter.lmr")
    $communityConsumer = Join-Path $consumerRoot "main.lmr"
    $communitySource = "from Greeter import greet`nbegin`n    writeln(greet(`"Portable`"))`nend.`n"
    [System.IO.File]::WriteAllText($communityConsumer, $communitySource, [System.Text.UTF8Encoding]::new($false))
    $previousModulePath = $env:LMR_MODULE_PATH
    try {
        $env:LMR_MODULE_PATH = $moduleRoot
        $communityOutput = & (Join-Path $workDirFull "lmr.exe") check $communityConsumer 2>&1 | Out-String
        if ($LASTEXITCODE -ne 0) {
            throw "No se pudo importar un módulo comunitario desde LMR_MODULE_PATH: $communityOutput"
        }
    } finally {
        if ($null -eq $previousModulePath) {
            Remove-Item Env:LMR_MODULE_PATH -ErrorAction SilentlyContinue
        } else {
            $env:LMR_MODULE_PATH = $previousModulePath
        }
    }
    $userDataRoot = Join-Path $runDir "user-data"
    $userModules = Join-Path $userDataRoot "LMR\modules"
    New-Item -ItemType Directory -Path $userModules -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $workDirFull "examples\community\Greeter.lmr") -Destination (Join-Path $userModules "Greeter.lmr")
    $previousLocalAppData = $env:LOCALAPPDATA
    try {
        $env:LOCALAPPDATA = $userDataRoot
        Remove-Item Env:LMR_MODULE_PATH -ErrorAction SilentlyContinue
        $userModuleOutput = & (Join-Path $workDirFull "lmr.exe") check $communityConsumer 2>&1 | Out-String
        if ($LASTEXITCODE -ne 0) {
            throw "No se pudo importar un módulo desde la carpeta LMR del usuario: $userModuleOutput"
        }
    } finally {
        $env:LOCALAPPDATA = $previousLocalAppData
        if ($null -eq $previousModulePath) {
            Remove-Item Env:LMR_MODULE_PATH -ErrorAction SilentlyContinue
        } else {
            $env:LMR_MODULE_PATH = $previousModulePath
        }
    }

    $programPath = Join-Path $runDir "portable_smoke.lmr"
    $program = @'
from Math import raiz

begin
    writeln(raiz(25.0))
end.
'@
    [System.IO.File]::WriteAllText($programPath, $program, [System.Text.UTF8Encoding]::new($false))

    Push-Location $runDir
    try {
        $portableCompiler = Join-Path $workDirFull "lmr.exe"
        $output = & $portableCompiler run $programPath 2>&1 | Out-String
        if ($LASTEXITCODE -ne 0) {
            throw "El ejecutable portable terminó con código ${LASTEXITCODE}: $output"
        }
        if ($output -notmatch "5\.00") {
            throw "El programa portable no produjo el resultado esperado. Salida: $output"
        }

        $objectPath = Join-Path $runDir "portable_smoke.o"
        $objectOutput = & $portableCompiler $programPath -c -o $objectPath 2>&1 | Out-String
        if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $objectPath -PathType Leaf)) {
            throw "No se pudo generar el objeto nativo: $objectOutput"
        }

        $beforeChecks = @(Get-ChildItem -LiteralPath $runDir -File | Select-Object -ExpandProperty Name | Sort-Object)
        $fileCheck = & $portableCompiler check $programPath 2>&1 | Out-String
        if ($LASTEXITCODE -ne 0) { throw "lmr check rechazó el programa válido: $fileCheck" }
        $sourceText = [System.IO.File]::ReadAllText($programPath)
        $stdinCheck = $sourceText | & $portableCompiler check --stdin $programPath 2>&1 | Out-String
        if ($LASTEXITCODE -ne 0) { throw "lmr check --stdin rechazó el programa válido: $stdinCheck" }
        $invalidText = "begin`nint invalid`nend`n"
        $invalidCheck = $invalidText | & $portableCompiler check --stdin $programPath 2>&1 | Out-String
        if ($LASTEXITCODE -eq 0) { throw "lmr check --stdin aceptó un programa inválido: $invalidCheck" }
        $invalidPath = Join-Path $runDir "invalid.lmr"
        [System.IO.File]::WriteAllText($invalidPath, $invalidText, [System.Text.UTF8Encoding]::new($false))
        $invalidCompile = & $portableCompiler $invalidPath 2>&1 | Out-String
        if ($LASTEXITCODE -eq 0) { throw "El compilador indicó éxito con fuente inválida: $invalidCompile" }
        Remove-Item -LiteralPath $invalidPath
        $afterChecks = @(Get-ChildItem -LiteralPath $runDir -File | Select-Object -ExpandProperty Name | Sort-Object)
        if (Compare-Object $beforeChecks $afterChecks) { throw "lmr check generó archivos inesperados" }

        & (Join-Path $workDirFull "install-windows.ps1") -InstallRoot $installRoot -NoPath
        $version = (Get-Content -LiteralPath (Join-Path $workDirFull "VERSION") -Raw).Trim()
        $installedExe = Join-Path (Join-Path $installRoot $version) "lmr.exe"
        $versionOutput = & $installedExe --version 2>&1 | Out-String
        if ($LASTEXITCODE -ne 0 -or $versionOutput -notmatch [regex]::Escape($version)) {
            throw "El compilador instalado no informa su versión. Salida: $versionOutput"
        }

        try {
            & (Join-Path $workDirFull "install-windows.ps1") -InstallRoot $userInstallRoot
            $userInstalledPath = Join-Path $userInstallRoot $version
            $userInstalledExe = Join-Path $userInstalledPath "lmr.exe"
            $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
            $userPathChanged = $userPath -ne $originalUserPath
            if (($userPath -split ";") -notcontains $userInstalledPath) {
                throw "La instalación no agregó su carpeta al PATH de usuario"
            }
            $userResolvedCommand = Get-Command lmr.exe -ErrorAction Stop
            if ([System.IO.Path]::GetFullPath($userResolvedCommand.Source) -ne [System.IO.Path]::GetFullPath($userInstalledExe)) {
                throw "PATH de usuario no resuelve al compilador instalado: $($userResolvedCommand.Source)"
            }
            $userVersionOutput = & lmr.exe --version 2>&1 | Out-String
            if ($LASTEXITCODE -ne 0 -or $userVersionOutput -notmatch [regex]::Escape($version)) {
                throw "No se pudo ejecutar lmr desde el PATH de usuario: $userVersionOutput"
            }
        } catch {
            if ($env:CI -or $_.Exception.Message -notmatch "Requested registry access is not allowed") {
                throw
            }
            Write-Warning "Se omitió la prueba del PATH de usuario: este entorno aislado no permite escribir en el registro."
        }
        $env:Path = $originalProcessPath

        $staleVersionPath = Join-Path $pathInstallRoot "0.1.0-alpha.9"
        $env:Path = $staleVersionPath + ";" + $originalProcessPath
        & (Join-Path $workDirFull "install-windows.ps1") -InstallRoot $pathInstallRoot -PathScope Process
        $pathInstalledExe = Join-Path (Join-Path $pathInstallRoot $version) "lmr.exe"
        $processPathEntries = @($env:Path -split ";")
        if ([System.IO.Path]::GetFullPath($processPathEntries[0]) -ne [System.IO.Path]::GetFullPath((Join-Path $pathInstallRoot $version))) {
            throw "La nueva versión no quedó primera en el PATH del proceso"
        }
        if ($processPathEntries -contains $staleVersionPath) {
            throw "El instalador no retiró la versión anterior de PATH"
        }
        $resolvedCommand = Get-Command lmr.exe -ErrorAction Stop
        if ([System.IO.Path]::GetFullPath($resolvedCommand.Source) -ne [System.IO.Path]::GetFullPath($pathInstalledExe)) {
            throw "PATH no resuelve al compilador de esta instalación: $($resolvedCommand.Source)"
        }
        $pathVersionOutput = & lmr.exe --version 2>&1 | Out-String
        if ($LASTEXITCODE -ne 0 -or $pathVersionOutput -notmatch [regex]::Escape($version)) {
            throw "No se pudo ejecutar lmr desde PATH: $pathVersionOutput"
        }
        & (Join-Path $workDirFull "uninstall-windows.ps1") -InstallRoot $pathInstallRoot -PathScope Process -Confirm:$false
        if (Test-Path -LiteralPath (Join-Path $pathInstallRoot $version)) {
            throw "El desinstalador no eliminó la carpeta de LMR instalada"
        }
        if (($env:Path -split ";") -contains (Join-Path $pathInstallRoot $version)) {
            throw "El desinstalador no retiró LMR del PATH del proceso"
        }
        & (Join-Path $workDirFull "uninstall-windows.ps1") -InstallRoot $installRoot -NoPath -Confirm:$false
        if (Test-Path -LiteralPath (Join-Path $installRoot $version)) {
            throw "El desinstalador no eliminó la instalación sin PATH"
        }
        Write-Output "Smoke correcto: paquete, check, actualización de PATH, instalación y desinstalación funcionaron fuera del checkout."
    } finally {
        Pop-Location
    }
} finally {
    try {
        if ($userPathChanged) {
            [Environment]::SetEnvironmentVariable("Path", $originalUserPath, "User")
        }
    } finally {
        $env:Path = $originalProcessPath
        foreach ($cleanupPath in @($pathInstallRoot, $userInstallRoot, $installRoot, $runDir, $workDirFull)) {
            if (-not (Test-Path -LiteralPath $cleanupPath)) { continue }
            $resolvedTemp = (Resolve-Path -LiteralPath $tempRoot).Path.TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
            $resolvedCleanup = (Resolve-Path -LiteralPath $cleanupPath).Path
            if (-not $resolvedCleanup.StartsWith($resolvedTemp, [System.StringComparison]::OrdinalIgnoreCase)) {
                throw "Se rechazó limpiar una ruta fuera del directorio temporal: $resolvedCleanup"
            }
            Remove-Item -LiteralPath $resolvedCleanup -Recurse -Force
        }
    }
}
