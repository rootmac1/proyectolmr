param(
    [string]$CompilerPath = (Join-Path $PSScriptRoot '..\target\debug\lmr.exe')
)

$ErrorActionPreference = 'Stop'
$compiler = (Resolve-Path -LiteralPath $CompilerPath).Path
$classes = 'HKCU:\Software\Classes'
$extensionPath = "$classes\.lmr"
$iconKeyPath = "$extensionPath\DefaultIcon"
$hadIconKey = Test-Path -LiteralPath $iconKeyPath
$previousIcon = ''
if ($hadIconKey) {
    $iconKey = Get-Item -LiteralPath $iconKeyPath
    $previousIcon = [string]$iconKey.GetValue('')
}

$backup = 'HKCU:\Software\LMR\AssociationBackup'
if (-not (Test-Path -LiteralPath $backup)) {
    New-Item -Path $backup -Force | Out-Null
    New-ItemProperty -Path $backup -Name HadIconKey -Value $hadIconKey -PropertyType DWord -Force | Out-Null
    New-ItemProperty -Path $backup -Name PreviousIcon -Value $previousIcon -PropertyType String -Force | Out-Null
}

New-Item -Path $iconKeyPath -Force | Out-Null
Set-Item -Path $iconKeyPath -Value "$compiler,0"

# Also expose an explicit context-menu action, which remains available when
# Windows has a protected UserChoice association for another editor.
$verb = "$classes\SystemFileAssociations\.lmr\shell\LMR.CompileRun"
New-Item -Path "$verb\command" -Force | Out-Null
Set-Item -Path $verb -Value 'Compilar y ejecutar con LMR'
New-ItemProperty -Path $verb -Name Icon -Value "$compiler,0" -PropertyType String -Force | Out-Null
Set-Item -Path "$verb\command" -Value ('"{0}" run "%1"' -f $compiler)

Write-Host 'Icono de archivos .lmr registrado para el usuario actual.'
Write-Host 'El menú contextual añade la acción Compilar y ejecutar con LMR; la aplicación predeterminada para abrir .lmr no se modifica.'
