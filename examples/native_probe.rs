//! Writes test hives and edits disposable offline Windows hives for native validation.
use regf_rs::{Hive, RegError, RegValue};
use std::{env, error::Error, path::Path};

fn values(hive: &mut Hive, path: &str, marker: &str) -> Result<(), RegError> {
    hive.create_key(path)?;
    hive.set_value(path, "Marker", RegValue::Sz(marker.into()))?;
    hive.set_value(
        path,
        "Expanded",
        RegValue::ExpandSz("%SystemRoot%\\regf-native.bmp".into()),
    )?;
    hive.set_value(path, "Dword", RegValue::Dword(0x12345678))?;
    hive.set_value(path, "BigEndian", RegValue::DwordBigEndian(0x12345678))?;
    hive.set_value(path, "Qword", RegValue::Qword(0x123456789abcdef0))?;
    hive.set_value(
        path,
        "Multi",
        RegValue::MultiSz(vec!["first".into(), "second".into()]),
    )?;
    hive.set_value(path, "Binary", RegValue::Binary(vec![0x5a; 16344]))?;
    hive.set_value(
        path,
        "Link",
        RegValue::Link(
            "\\Registry\\Machine\\SOFTWARE"
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect(),
        ),
    )?;
    hive.set_value(path, "Deleted", RegValue::Sz("remove me".into()))?;
    hive.delete_value(path, "Deleted")?;
    hive.set_value(path, "Replaced", RegValue::Binary(vec![1; 4096]))?;
    hive.set_value(path, "Replaced", RegValue::Dword(42))?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() < 3 {
        return Err(
            "usage: native_probe new OUTPUT | software|system|bcd INPUT OUTPUT MARKER".into(),
        );
    }
    let mode = args[1].to_str().ok_or("invalid mode")?;
    if mode == "new" {
        let mut hive = Hive::new_empty("根");
        values(&mut hive, "NativeValidation", "regf-native-new")?;
        hive.save(&args[2])?;
    } else {
        if args.len() != 5 {
            return Err("expected INPUT OUTPUT MARKER".into());
        }
        let input = Path::new(&args[2]);
        let output = Path::new(&args[3]);
        if input == output {
            return Err("use a separate output path".into());
        }
        let marker = args[4].to_str().ok_or("invalid marker")?;
        let mut hive = Hive::from_file(input)?;
        match mode {
            "software" => {
                values(&mut hive, "RegfNativeValidation", marker)?;
                let winpe = "Microsoft\\Windows NT\\CurrentVersion\\WinPE";
                hive.create_key(winpe)?;
                hive.set_value(
                    winpe,
                    "CustomBackground",
                    RegValue::ExpandSz("%SystemRoot%\\regf-native.bmp".into()),
                )?;
                match hive.delete_value(winpe, "CustomShell") {
                    Ok(()) | Err(RegError::ValueNotFound(_)) => {}
                    Err(error) => return Err(error.into()),
                }
            }
            "system" => {
                let RegValue::Dword(current) = hive.get_value("Select", "Current")? else {
                    return Err("SYSTEM Select\\Current is not a DWORD".into());
                };
                values(
                    &mut hive,
                    &format!("ControlSet{current:03}\\Control\\RegfNativeValidation"),
                    marker,
                )?;
            }
            "bcd" => {
                let bootmgr = "Objects\\{9dea862c-5cdd-4e70-acc1-f32b344d4795}\\Elements";
                let RegValue::Sz(object) =
                    hive.get_value(&format!("{bootmgr}\\23000003"), "Element")?
                else {
                    return Err("BCD default object is not a string GUID".into());
                };
                hive.open(&format!("Objects\\{object}"))?;
                // OS-loader descriptions persist independently of firmware boot labels.
                let path = format!("Objects\\{object}\\Elements\\12000004");
                hive.create_key(&path)?;
                hive.set_value(&path, "Element", RegValue::Sz(marker.into()))?;
                let timeout = format!("{bootmgr}\\25000004");
                hive.create_key(&timeout)?;
                hive.set_value(
                    &timeout,
                    "Element",
                    RegValue::Binary(7u64.to_le_bytes().to_vec()),
                )?;
            }
            _ => return Err("unknown mode".into()),
        }
        hive.save(output)?;
    }
    println!("regf native probe: {mode} complete");
    Ok(())
}
