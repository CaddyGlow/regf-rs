# Native Windows validation, 2026-10-04 through 2026-10-05

Result: **passed for the exercised operations on Windows 11 Pro build 26200,
with UEFI Secure Boot enabled**. The final boot time reported by Windows was
2026-10-05T03:46:36.5000000Z. See `summary.json` and `boot-results.json`.

## Procedure and evidence

- `native-results.json` and `native-transcript.txt`: Windows 11 loaded a fresh
  Rust-created hive with a Unicode root name. `RegQueryValueExW` verified nine
  values' types and bytes, including REG_LINK, big-endian DWORD, MULTI_SZ and a
  16,344-byte binary. Deletion and replacement were checked, Windows wrote a
  DWORD, and the hive unloaded successfully.
- `offline-results.json`, `offline-transcript.txt`, `bcd-native-enum.txt`: a
  Windows 10 Pro build 19045 helper edited copies of the stopped target's
  SOFTWARE, SYSTEM and BCD files using the Rust probe. Native load/query/unload
  verified SOFTWARE/SYSTEM copies; `bcdedit /store` verified a BCD copy. The
  original generated files were not passed through a native loader before
  installation.
- `installed-hashes.json`: installed SOFTWARE, SYSTEM and BCD SHA-256 hashes
  match the generated output hashes in `offline-results.json`.
- `boot-results.json` and `booted-bcd-enum.txt`: the target booted from the edited
  disk with Secure Boot enabled. All 18 checked SOFTWARE/active-SYSTEM values
  matched, WinPE CustomBackground and CustomShell removal persisted, the active
  Windows loader description matched the marker, and the boot timeout was seven
  seconds.
- `native-write-readback-tests.txt`: the hive modified and unloaded by Windows
  was read and edited successfully on Linux, cross-checked with `nt-hive`.
- `host-tests.txt` and `no-std-tests.txt`: regression suites passed after the
  additional allocation fix. Formatting and Clippy also passed with default and
  no-std configurations.

Text artifacts have normalized line endings and trailing whitespace; the hive
and executable hashes refer to the original binary files.

The executable SHA-256 is recorded in both native and offline result JSON files.
The probe and PowerShell scripts are in `examples/native_probe.rs` and
`scripts/native-validation/`; their README contains reproduction steps.

## Additional defect found by the native gate

The original SOFTWARE file was 76,808,192 bytes long, while the end of its
header-declared hive-bin region was 76,410,880. The allocator appended new bins
after the physical EOF instead of the declared region, leaving the large value
outside the valid bin region. Native loading succeeded, but the binary value
was missing afterward (`padding-native-before.txt`).

The allocator now grows at the declared bin boundary, consumes existing file
padding when available, and rejects truncated or crossing bins. The focused
regression failed before the fix (`padding-regression-before.txt`) and passes
in the retained host suites. Native verification then found the full binary
value before and after boot.

## Scope

Two disposable overlays were used (`regf-native-win11-1004` from `win11-secure`,
`regf-native-helper-1004` from `win10-dev`), each with 4 vCPUs and 4 GiB RAM.
Both VMs were stopped after testing. Shared base images were not modified;
overlays and source hives remain local recovery evidence and are not included
in this repository.

The first boot experiment changed the boot-manager description, which reverted
on boot. The final gate instead requires the OS-loader description and timeout
and uses a new marker in all three hives. Probe setup was also corrected to use
BCD's native REG_SZ representation for description and object-reference elements.

This is native compatibility evidence for these operations and builds, not a
claim covering all Windows versions, BCD elements, Unicode lookup cases, or a
complete Windows installation/WinPE ISO servicing gate. An additional full-tree
cross-reader run on the 77 MB SOFTWARE hive was cancelled after extended runtime
and is not counted as passed. The retained cross-reader readback pass concerns
the smaller native-written test hive.
