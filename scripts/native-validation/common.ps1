Add-Type @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public class NativeRegistryValue { public uint Type; public byte[] Data; }
public static class NativeRegistry {
    [DllImport("advapi32.dll", CharSet=CharSet.Unicode)] static extern int RegOpenKeyEx(IntPtr key, string subkey, uint options, uint access, out IntPtr result);
    [DllImport("advapi32.dll", CharSet=CharSet.Unicode)] static extern int RegQueryValueEx(IntPtr key, string name, IntPtr reserved, out uint type, byte[] data, ref uint size);
    [DllImport("advapi32.dll")] static extern int RegCloseKey(IntPtr key);
    public static NativeRegistryValue Read(string path, string name) {
        IntPtr key; int error=RegOpenKeyEx(new IntPtr(unchecked((int)0x80000002)), path, 0, 0x20019, out key);
        if(error!=0) throw new Win32Exception(error);
        try {
            uint type, size=0;
            error=RegQueryValueEx(key,name,IntPtr.Zero,out type,null,ref size);
            if(error!=0) throw new Win32Exception(error);
            byte[] data=new byte[size];
            error=RegQueryValueEx(key,name,IntPtr.Zero,out type,data,ref size);
            if(error!=0) throw new Win32Exception(error);
            return new NativeRegistryValue {Type=type,Data=data};
        } finally { RegCloseKey(key); }
    }
}
'@
function Run-Reg([string[]]$Arguments) {
    $output = & reg.exe @Arguments 2>&1
    $code = $LASTEXITCODE
    $output | Out-Host
    if ($code -ne 0) { throw "reg.exe $Arguments failed: $code" }
}
function Assert-Value([string]$Key,[string]$Name,[uint32]$Type,[byte[]]$Bytes) {
    $got = [NativeRegistry]::Read($Key,$Name)
    if ($got.Type -ne $Type -or [Convert]::ToBase64String($got.Data) -cne [Convert]::ToBase64String($Bytes)) {
        throw "Native value differs: $Key\$Name (type=$($got.Type), length=$($got.Data.Length))"
    }
    [pscustomobject]@{name=$Name; type=$got.Type; length=$got.Data.Length; matched=$true}
}
function Test-Values([string]$Key,[string]$Marker) {
    Assert-Value $Key 'Marker' 1 ([Text.Encoding]::Unicode.GetBytes($Marker + [char]0))
    Assert-Value $Key 'Expanded' 2 ([Text.Encoding]::Unicode.GetBytes('%SystemRoot%\regf-native.bmp' + [char]0))
    Assert-Value $Key 'Dword' 4 ([BitConverter]::GetBytes([uint32]0x12345678))
    Assert-Value $Key 'BigEndian' 5 ([byte[]](0x12,0x34,0x56,0x78))
    Assert-Value $Key 'Qword' 11 ([BitConverter]::GetBytes([uint64]0x123456789abcdef0))
    Assert-Value $Key 'Multi' 7 ([Text.Encoding]::Unicode.GetBytes('first' + [char]0 + 'second' + [char]0 + [char]0))
    Assert-Value $Key 'Binary' 3 ([byte[]](1..16344 | ForEach-Object {0x5a}))
    Assert-Value $Key 'Link' 6 ([Text.Encoding]::Unicode.GetBytes('\Registry\Machine\SOFTWARE'))
    Assert-Value $Key 'Replaced' 4 ([BitConverter]::GetBytes([uint32]42))
    $keyObject = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey($Key)
    try { if ($keyObject.GetValueNames() -contains 'Deleted') { throw 'Deleted value still present' } }
    finally { $keyObject.Close() }
}
