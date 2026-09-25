//! Load arbitrary bytes as a hive, walk every key and value, then edit it and
//! write it back: reading or writing may fail, nothing may panic or hang.
//!
//! A hive seen by a bootloader or a forensic tool comes from anywhere, so
//! every offset in it is attacker-controlled.

#![no_main]

use libfuzzer_sys::fuzz_target;
use regf_rs::{Hive, RegValue};

/// A corrupt hive can describe a cycle between keys, or a key that lists
/// itself many times. The walk stops at this depth and after this many keys in
/// total, so the harness itself never runs away; a loop inside the library
/// still shows up as a timeout.
const MAX_DEPTH: usize = 16;
const MAX_KEYS: usize = 4096;

fn walk(hive: &Hive, path: &str, depth: usize, budget: &mut usize) {
    if depth > MAX_DEPTH || *budget == 0 {
        return;
    }
    *budget -= 1;
    let _ = hive.list_values(path);
    if let Ok(keys) = hive.list_subkeys(path) {
        for key in &keys {
            if *budget == 0 {
                return;
            }
            let child = if path.is_empty() { key.clone() } else { format!("{path}\\{key}") };
            walk(hive, &child, depth + 1, budget);
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let Ok(mut hive) = Hive::from_bytes(data.to_vec()) else {
        return;
    };
    let _ = hive.root_key();
    walk(&hive, "", 0, &mut MAX_KEYS.clone());
    if hive.create_key("Fuzz").is_ok() {
        let _ = hive.set_value("Fuzz", "v", RegValue::Dword(1));
        let _ = hive.delete_value("Fuzz", "v");
    }
    let _ = hive.to_bytes();
});
