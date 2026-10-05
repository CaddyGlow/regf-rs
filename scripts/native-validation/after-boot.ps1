param([string]$Directory='C:\regf-native',[string]$Marker='regf-native-20261005-final')
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot 'common.ps1')
$software = @(Test-Values 'SOFTWARE\RegfNativeValidation' $Marker)
$system = @(Test-Values 'SYSTEM\CurrentControlSet\Control\RegfNativeValidation' $Marker)
$winpe = 'SOFTWARE\Microsoft\Windows NT\CurrentVersion\WinPE'
Assert-Value $winpe 'CustomBackground' 2 ([Text.Encoding]::Unicode.GetBytes('%SystemRoot%\regf-native.bmp' + [char]0)) | Out-Null
$regKey = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey($winpe)
try { if ($regKey.GetValueNames() -contains 'CustomShell') { throw 'CustomShell deletion did not persist' } } finally { $regKey.Close() }
$bcd = & bcdedit.exe /enum '{current}' 2>&1
if ($LASTEXITCODE -ne 0 -or "$bcd" -notmatch [regex]::Escape($Marker)) { throw "Booted BCD marker missing: $bcd" }
$bootmgr = & bcdedit.exe /enum '{bootmgr}' 2>&1
if ($LASTEXITCODE -ne 0 -or "$bootmgr" -notmatch 'timeout\s+7(?:\s|$)') { throw "Booted BCD timeout missing: $bootmgr" }
$bcd = @($bcd) + @($bootmgr)
$bcd | Set-Content (Join-Path $Directory 'booted-bcd-enum.txt') -Encoding UTF8
$os = Get-CimInstance Win32_OperatingSystem
[pscustomobject]@{
    os=$os.Caption; version=$os.Version; build=$os.BuildNumber; last_boot_utc=$os.LastBootUpTime.ToUniversalTime().ToString('o');
    secure_boot=(Confirm-SecureBootUEFI); marker=$Marker; software=$software; system=$system;
    winpe_configuration_persisted=$true; bcd_marker_matched=$true; bcd_timeout=7; guest_agent_after_boot=$true
} | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $Directory 'boot-results.json') -Encoding UTF8
Get-Content (Join-Path $Directory 'boot-results.json')
