param([string]$HostExecutable)
$parameters = @{}
if ($HostExecutable) { $parameters.HostExecutable = $HostExecutable }
& (Join-Path $PSScriptRoot 'install-native-host.ps1') @parameters
if (!$?) { exit 1 }
