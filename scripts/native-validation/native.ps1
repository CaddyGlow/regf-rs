param([string]$Directory = 'C:\regf-native')
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
New-Item -ItemType Directory -Force $Directory | Out-Null
Start-Transcript -Path (Join-Path $Directory 'native-transcript.txt') -Force | Out-Null
. (Join-Path $PSScriptRoot 'common.ps1')
$exe = Join-Path $Directory 'native_probe.exe'
$fresh = Join-Path $Directory 'fresh-unicode.hive'
& $exe new $fresh
if ($LASTEXITCODE -ne 0) { throw 'fresh hive creation failed' }
$freshHash = (Get-FileHash $fresh -Algorithm SHA256).Hash
$key = 'REGF_NATIVE_FRESH'
Run-Reg @('load',"HKLM\$key",$fresh)
try {
    $values = @(Test-Values "$key\NativeValidation" 'regf-native-new')
    Run-Reg @('add',"HKLM\$key\NativeValidation",'/v','NativeWritten','/t','REG_DWORD','/d','77','/f')
    Assert-Value "$key\NativeValidation" 'NativeWritten' 4 ([BitConverter]::GetBytes([uint32]77)) | Out-Null
} finally { Run-Reg @('unload',"HKLM\$key") }
$os = Get-CimInstance Win32_OperatingSystem
[pscustomobject]@{
    os=$os.Caption; version=$os.Version; build=$os.BuildNumber;
    secure_boot=(Confirm-SecureBootUEFI); binary_sha256=(Get-FileHash $exe -Algorithm SHA256).Hash; fresh_hive_sha256=$freshHash;
    native_load=$true; values=$values; native_write=$true; native_unload=$true;
} | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Directory 'native-results.json') -Encoding UTF8
Stop-Transcript | Out-Null
