param(
    [Parameter(Mandatory=$true)][string]$WindowsRoot,
    [Parameter(Mandatory=$true)][string]$EspRoot,
    [string]$Directory = 'C:\regf-native',
    [string]$Marker = 'regf-native-20261005-final'
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ([IO.Path]::GetPathRoot([IO.Path]::GetFullPath($WindowsRoot)) -eq "$env:SystemDrive\") {
    throw 'Refusing to edit the helper OS: supply a disposable offline Windows volume'
}
New-Item -ItemType Directory -Force $Directory | Out-Null
Start-Transcript -Path (Join-Path $Directory 'offline-transcript.txt') -Force | Out-Null
. (Join-Path $PSScriptRoot 'common.ps1')
$exe = Join-Path $Directory 'native_probe.exe'
$cases = @(
    @{mode='software'; target=(Join-Path $WindowsRoot 'Windows\System32\config\SOFTWARE')},
    @{mode='system'; target=(Join-Path $WindowsRoot 'Windows\System32\config\SYSTEM')},
    @{mode='bcd'; target=(Join-Path $EspRoot 'EFI\Microsoft\Boot\BCD')}
)
$results = @()
foreach ($case in $cases) {
    $mode = $case.mode
    $original = Join-Path $Directory "$mode.original"
    $output = Join-Path $Directory "$mode.edited"
    $check = Join-Path $Directory "$mode.native-check"
    if (Test-Path $original) { throw "Original already retained: $original" }
    Copy-Item -LiteralPath $case.target -Destination $original
    $sourceHash = (Get-FileHash $original -Algorithm SHA256).Hash
    & $exe $mode $original $output $Marker
    if ($LASTEXITCODE -ne 0) { throw "Rust edit failed: $mode (exit $LASTEXITCODE)" }
    $editedHash = (Get-FileHash $output -Algorithm SHA256).Hash
    Copy-Item -LiteralPath $output -Destination $check
    if ($mode -eq 'bcd') {
        $listing = & bcdedit.exe /store $check /enum '{default}' 2>&1
        if ($LASTEXITCODE -ne 0 -or "$listing" -notmatch [regex]::Escape($Marker)) { throw "BCD verification failed: $listing" }
        $bootmgr = & bcdedit.exe /store $check /enum '{bootmgr}' 2>&1
        if ($LASTEXITCODE -ne 0 -or "$bootmgr" -notmatch 'timeout\s+7(?:\s|$)') { throw "BCD timeout verification failed: $bootmgr" }
        $listing = @($listing) + @($bootmgr)
        $listing | Set-Content (Join-Path $Directory 'bcd-native-enum.txt') -Encoding UTF8
        $validation = @([pscustomobject]@{bcdedit=$true; marker=$Marker})
    } else {
        $key = "REGF_OFFLINE_$mode"
        Run-Reg @('load',"HKLM\$key",$check)
        try {
            if ($mode -eq 'software') {
                $path = "$key\RegfNativeValidation"
                $winpe = "$key\Microsoft\Windows NT\CurrentVersion\WinPE"
                Assert-Value $winpe 'CustomBackground' 2 ([Text.Encoding]::Unicode.GetBytes('%SystemRoot%\regf-native.bmp' + [char]0)) | Out-Null
                $regKey = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey($winpe)
                try { if ($regKey.GetValueNames() -contains 'CustomShell') { throw 'CustomShell deletion failed' } } finally { $regKey.Close() }
            } else {
                $current = [BitConverter]::ToUInt32([NativeRegistry]::Read("$key\Select",'Current').Data,0)
                $path = "$key\ControlSet$('{0:000}' -f $current)\Control\RegfNativeValidation"
            }
            $validation = @(Test-Values $path $Marker)
        } finally { Run-Reg @('unload',"HKLM\$key") }
    }
    if ((Get-FileHash $output -Algorithm SHA256).Hash -ne $editedHash) { throw 'Validation changed original output' }
    Copy-Item -LiteralPath $output -Destination $case.target -Force
    $installedHash = (Get-FileHash $case.target -Algorithm SHA256).Hash
    if ($installedHash -ne $editedHash) { throw "Installed bytes differ: $mode" }
    $results += [pscustomobject]@{mode=$mode; source_sha256=$sourceHash; edited_sha256=$editedHash; installed_sha256=$installedHash; native_validation=$validation}
}
[pscustomobject]@{marker=$Marker; helper_os=(Get-CimInstance Win32_OperatingSystem).Caption; cases=$results; binary_sha256=(Get-FileHash $exe -Algorithm SHA256).Hash} |
    ConvertTo-Json -Depth 10 | Set-Content (Join-Path $Directory 'offline-results.json') -Encoding UTF8
Stop-Transcript | Out-Null
