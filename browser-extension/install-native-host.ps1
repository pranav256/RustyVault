[CmdletBinding(SupportsShouldProcess)]
param([string]$HostExecutable)

$ErrorActionPreference = 'Stop'
if (!$HostExecutable) {
    $HostExecutable = Join-Path $PSScriptRoot '..\rustyvault-bridge.exe'
    if (!(Test-Path -LiteralPath $HostExecutable -PathType Leaf)) {
        $HostExecutable = Join-Path $PSScriptRoot '..\target\release\rustyvault-bridge.exe'
    }
}
$hostPath = (Resolve-Path -LiteralPath $HostExecutable).Path
if (!(Test-Path -LiteralPath $hostPath -PathType Leaf) -or [IO.Path]::GetExtension($hostPath) -ne '.exe') {
    throw 'Choose the built rustyvault-bridge.exe executable.'
}
$directory = Join-Path $PSScriptRoot 'native-hosts'
$chromiumPath = Join-Path $directory 'chromium.json'
$firefoxPath = Join-Path $directory 'firefox.json'
$chromium = [ordered]@{
    name = 'com.pranav256.rustyvault'
    description = 'RustyVault browser connection'
    path = $hostPath
    type = 'stdio'
    allowed_origins = @('chrome-extension://mbogmhfmgcaindpokcpgimbaefjoppgb/')
}
$firefox = [ordered]@{
    name = $chromium.name
    description = $chromium.description
    path = $hostPath
    type = 'stdio'
    allowed_extensions = @('rustyvault@pranav256.com')
}
if ($PSCmdlet.ShouldProcess('Current Windows user', 'Register the RustyVault native host for Firefox, Chrome, Edge and Brave')) {
    $null = New-Item -ItemType Directory -Path $directory -Force
    $encoding = [Text.UTF8Encoding]::new($false)
    [IO.File]::WriteAllText($chromiumPath, ($chromium | ConvertTo-Json), $encoding)
    [IO.File]::WriteAllText($firefoxPath, ($firefox | ConvertTo-Json), $encoding)
    foreach ($vendor in @('Google\Chrome', 'Microsoft\Edge', 'BraveSoftware\Brave-Browser')) {
        $registryPath = "HKCU:\Software\$vendor\NativeMessagingHosts\com.pranav256.rustyvault"
        $null = New-Item -Path $registryPath -Force
        Set-Item -LiteralPath $registryPath -Value $chromiumPath
    }
    $mozilla = 'HKCU:\Software\Mozilla\NativeMessagingHosts\com.pranav256.rustyvault'
    $null = New-Item -Path $mozilla -Force
    Set-Item -LiteralPath $mozilla -Value $firefoxPath
    Write-Output 'Registered RustyVault for Firefox, Chrome, Edge and Brave.'
    Write-Output "Load the extension from: $PSScriptRoot"
    Write-Output 'Firefox: about:debugging#/runtime/this-firefox, Load Temporary Add-on, manifest.json.'
    Write-Output 'Chrome, Edge and Brave: extensions page, Developer mode, Load unpacked.'
}
