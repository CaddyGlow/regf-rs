# Native Windows hive validation

Run these checks on disposable Windows VMs. `native_probe` writes test values
and changes the selected image's WinPE configuration, SYSTEM control set and BCD.
All Rust edits go to a separate output file; `offline.ps1` retains originals,
checks copies through native Windows APIs, then installs the untouched outputs.

## Build

```sh
RUSTFLAGS='-C target-feature=+crt-static' cargo xwin build --locked --release \
  --example native_probe --target x86_64-pc-windows-msvc
```

Copy the executable and these PowerShell scripts to `C:\regf-native` in the test
VMs. The static CRT build avoids requiring the Visual C++ runtime in the guest.

## Native loading

Run an elevated Windows PowerShell prompt:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File C:\regf-native\native.ps1
```

This creates a hive with a Unicode root name, loads it with `reg.exe`, checks
nine values' exact types and bytes with `RegQueryValueExW`, checks deletion,
writes another value through Windows, and unloads the hive. Results and a
transcript are written beside the executable.

## Offline edits and boot

Shut down the target VM, and attach its disposable disk to a separate Windows
helper VM. Assign drive letters to its Windows and EFI system partitions. The
helper must not be booted from the target disk.

From an elevated prompt in the helper (D: and S: are examples):

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File C:\regf-native\offline.ps1 `
  -WindowsRoot D:\ -EspRoot S:\ -Marker regf-native-20261005-final
```

The script modifies SOFTWARE and SYSTEM, including a 16,344-byte value to force
allocation, and changes the default Windows loader description and boot-manager
timeout in BCD. Native checks run on **copies** of the generated outputs so that
native loading cannot repair the files installed for the boot test. Recorded
SHA-256 hashes must match between generated and installed files.

Copy `common.ps1` and `after-boot.ps1` to `C:\regf-native` on the target volume.
Retain the helper's `offline-results.json`, transcript and BCD enumeration.
Shut down the helper to release the disk, then boot the target VM normally.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File C:\regf-native\after-boot.ps1 `
  -Marker regf-native-20261005-final
```

A successful result requires all SOFTWARE and active SYSTEM-control-set values,
WinPE configuration, the running OS loader's BCD description, and the seven-second
boot timeout to match. The result also records the Windows version, boot time,
and Secure Boot state. A boot-manager description alone is not used as the
post-boot marker: in the initial run that description reverted during boot.

This validates the exercised Windows build, hive contents and edit operations.
It does not establish every BCD element, Windows version or registry encoding.
