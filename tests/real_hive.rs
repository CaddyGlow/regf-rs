//! Opt-in cross-reader validation of a Windows-produced hive.
//! REGF_TEST_HIVE=/path/to/hive cargo test --test real_hive -- --ignored
//! Editing is performed on an in-memory copy; the input file is never written.
mod common;
use regf_rs::{Hive, RegValue};

fn source() -> Vec<u8> {
    let path =
        std::env::var_os("REGF_TEST_HIVE").expect("set REGF_TEST_HIVE to a Windows-produced hive");
    std::fs::read(path).expect("read hive")
}

#[test]
#[ignore = "requires REGF_TEST_HIVE pointing to a Windows-produced hive"]
fn reads_real_hive_and_matches_nt_hive() {
    common::assert_matches_nt(&source());
}

#[test]
#[ignore = "requires REGF_TEST_HIVE pointing to a clean Windows-produced hive"]
fn edits_real_hive_and_preserves_unrelated_raw_values() {
    let bytes = source();
    let before = common::nt_inventory(&bytes);
    let key = "__regf_rs_roundtrip_test__";
    let mut hive = Hive::from_bytes(bytes).unwrap();
    assert!(!hive.is_dirty(), "supply a reconciled hive");
    assert!(
        matches!(hive.open(key), Err(regf_rs::RegError::KeyNotFound(_))),
        "test key already exists"
    );
    hive.create_key(key).unwrap();
    hive.set_value(
        key,
        "CustomBackground",
        RegValue::ExpandSz("%SystemRoot%\\background.bmp".into()),
    )
    .unwrap();
    hive.set_value(key, "CustomShell", RegValue::Sz("temporary".into()))
        .unwrap();
    hive.delete_value(key, "CustomShell").unwrap();
    hive.set_value(key, "Count", RegValue::Dword(42)).unwrap();
    // Force bin growth in typical hives, then replace the external data with inline data.
    hive.set_value(key, "Growing", RegValue::Binary(vec![0x5a; 16344]))
        .unwrap();
    hive.set_value(key, "Growing", RegValue::Binary(vec![1, 2, 3]))
        .unwrap();
    let bytes = hive.to_bytes().unwrap();
    common::assert_matches_nt(&bytes);
    let mut after = common::nt_inventory(&bytes);
    let added = after.remove(key).expect("created key missing");
    assert_eq!(added.len(), 3);
    assert_eq!(
        added["CustomBackground"],
        (
            2,
            RegValue::ExpandSz("%SystemRoot%\\background.bmp".into()).to_bytes()
        )
    );
    assert_eq!(added["Count"], (4, 42u32.to_le_bytes().to_vec()));
    assert_eq!(added["Growing"], (3, vec![1, 2, 3]));
    assert_eq!(after, before, "unrelated keys and raw values changed");
}
