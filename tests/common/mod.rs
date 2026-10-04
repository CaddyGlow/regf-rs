//! Shared synthetic fixture: a BCD-like hive built in memory,
//! with no real machine data. Enables self-contained (CI) and
//! reproducible tests, without versioning any personal hive.

#![allow(dead_code)]

use regf_rs::{Hive, RegValue};

/// Fixed Windows Boot Manager GUID (a public Microsoft constant, not tied
/// to any machine).
pub const BOOTMGR: &str = "{9dea862c-5cdd-4e70-acc1-f32b344d4795}";
/// Neutral GUID acting as the default "OS loader" (made up).
pub const OSLOADER: &str = "{11111111-2222-3333-4444-555555555555}";

/// Builds a BCD-like hive: `Objects\{bootmgr}\Elements\{23000003,
/// 24000001, 25000004}` filled as in a real BCD, plus an OS loader
/// object. No identifying value.
pub fn synthetic_bcd() -> Hive {
    let mut h = Hive::new_empty("BCD");

    let bootmgr_elems = format!("Objects\\{BOOTMGR}\\Elements");
    h.create_key(&bootmgr_elems).unwrap();
    // DefaultObject → the OS loader.
    set(
        &mut h,
        &bootmgr_elems,
        "23000003",
        RegValue::Sz(OSLOADER.into()),
    );
    // DisplayOrder.
    set(
        &mut h,
        &bootmgr_elems,
        "24000001",
        RegValue::MultiSz(vec![OSLOADER.into()]),
    );
    // Timeout = 30 s.
    set(
        &mut h,
        &bootmgr_elems,
        "25000004",
        RegValue::Binary(vec![30, 0, 0, 0, 0, 0, 0, 0]),
    );

    // A minimal OS loader object.
    let os_elems = format!("Objects\\{OSLOADER}\\Elements");
    h.create_key(&os_elems).unwrap();
    set(
        &mut h,
        &os_elems,
        "12000004",
        RegValue::Sz("Windows".into()),
    );

    h
}

fn set(h: &mut Hive, elements: &str, code: &str, v: RegValue) {
    let path = format!("{elements}\\{code}");
    h.create_key(&path).unwrap();
    h.set_value(&path, "Element", v).unwrap();
}

/// A complete independent inventory: key paths, value names, types and raw bytes.
pub type Inventory =
    std::collections::BTreeMap<String, std::collections::BTreeMap<String, (u32, Vec<u8>)>>;

pub fn nt_inventory(bytes: &[u8]) -> Inventory {
    let nt = nt_hive::Hive::new(bytes).expect("independent parse");
    nt.validate().expect("independent header validation");
    let mut pending = vec![(String::new(), nt.root_key_node().unwrap())];
    let mut inventory = Inventory::new();
    while let Some((path, node)) = pending.pop() {
        assert!(inventory.len() < 1_000_000, "inventory key budget exceeded");
        let mut values = std::collections::BTreeMap::new();
        if let Some(iter) = node.values() {
            for value in iter.unwrap() {
                let value = value.unwrap();
                let name = value.name().unwrap().to_string();
                let ty = value.data_type().unwrap() as u32;
                let raw = value.data().unwrap().into_vec().unwrap();
                assert!(values.insert(name, (ty, raw)).is_none(), "duplicate value");
            }
        }
        assert!(
            inventory.insert(path.clone(), values).is_none(),
            "duplicate key"
        );
        if let Some(iter) = node.subkeys() {
            for child in iter.unwrap() {
                let child = child.unwrap();
                let name = child.name().unwrap().to_string();
                let child_path = if path.is_empty() {
                    name
                } else {
                    format!("{path}\\{name}")
                };
                pending.push((child_path, child));
            }
        }
    }
    inventory
}

pub fn assert_matches_nt(bytes: &[u8]) {
    let expected = nt_inventory(bytes);
    let hive = Hive::from_bytes(bytes.to_vec()).unwrap();
    let mut seen = std::collections::BTreeSet::new();
    let mut pending = vec![String::new()];
    while let Some(path) = pending.pop() {
        assert!(seen.insert(path.clone()), "duplicate key: {path}");
        let values = hive.list_values(&path).unwrap();
        let expected_values = expected
            .get(&path)
            .expect("key missing from independent reader");
        assert_eq!(values.len(), expected_values.len(), "value count at {path}");
        for (name, value) in values {
            let (ty, raw) = expected_values
                .get(&name)
                .expect("value missing from independent reader");
            let expected_value = RegValue::from_raw(regf_rs::RegType::from_u32(*ty), raw).unwrap();
            assert_eq!(value, expected_value, "value {path}\\{name}");
        }
        for child in hive.list_subkeys(&path).unwrap() {
            pending.push(if path.is_empty() {
                child
            } else {
                format!("{path}\\{child}")
            });
        }
    }
    assert_eq!(seen, expected.into_keys().collect(), "key inventory");
}
