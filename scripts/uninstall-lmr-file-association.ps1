$ErrorActionPreference = 'Stop'
$classes = 'HKCU:\Software\Classes'
$extensionPath = "$classes\.lmr"
$iconKeyPath = "$extensionPath\DefaultIcon"
$backup = 'HKCU:\Software\LMR\AssociationBackup'
$hadIconKey = $false
$previousIcon = ''
if (Test-Path -LiteralPath $backup) {
    $saved = Get-ItemProperty -LiteralPath $backup
    $hadIconKey = [bool]$saved.HadIconKey
    $previousIcon = [string]$saved.PreviousIcon
}
if (Test-Path -LiteralPath $iconKeyPath) {
    if ($hadIconKey) {
        Set-Item -LiteralPath $iconKeyPath -Value $previousIcon
    } else {
        Remove-Item -LiteralPath $iconKeyPath -Recurse -Force
    }
}
Remove-Item -LiteralPath "$classes\SystemFileAssociations\.lmr\shell\LMR.CompileRun" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath 'HKCU:\Software\LMR\AssociationBackup' -Recurse -Force -ErrorAction SilentlyContinue
$lmrSettings = 'HKCU:\Software\LMR'
if ((Test-Path -LiteralPath $lmrSettings) -and @(Get-ChildItem -LiteralPath $lmrSettings).Count -eq 0) {
    Remove-Item -LiteralPath $lmrSettings -Force
}
Write-Host 'Se quitaron la asociación por usuario y el menú contextual de LMR.'
