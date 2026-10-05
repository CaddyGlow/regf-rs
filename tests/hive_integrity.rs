use regf_rs::{Header, Hive, RegError, RegType, RegValue};
fn rd(b: &[u8], p: usize) -> u32 {
    u32::from_le_bytes(b[p..p + 4].try_into().unwrap())
}
fn wr(b: &mut [u8], p: usize, v: u32) {
    b[p..p + 4].copy_from_slice(&v.to_le_bytes());
}
fn vk(b: &[u8], index: usize) -> usize {
    let root = 4096 + rd(b, 0x24) as usize + 4;
    let list = 4096 + rd(b, root + 40) as usize + 4;
    4096 + rd(b, list + index * 4) as usize + 4
}
#[test]
fn serialization_must_not_clear_unreplayed_dirty_state() {
    let mut b = Hive::new_empty("ROOT").to_bytes().unwrap();
    let seq = rd(&b, 4);
    wr(&mut b, 8, seq + 1);
    let sum = Header::checksum(&b);
    wr(&mut b, 0x1fc, sum);
    let mut h = Hive::from_bytes(b).unwrap();
    assert!(h.is_dirty());
    assert_eq!(h.to_bytes(), Err(RegError::DirtyHive));
    assert!(h.is_dirty());
    assert_eq!(
        h.set_value("", "v", RegValue::Dword(1)),
        Err(RegError::DirtyHive)
    );
    assert!(matches!(h.create_key("child"), Err(RegError::DirtyHive)));
    assert_eq!(h.delete_value("", "v"), Err(RegError::DirtyHive));
}
#[test]
fn short_big_endian_value_must_not_panic() {
    let mut h = Hive::new_empty("ROOT");
    h.set_value(
        "",
        "bad",
        RegValue::Other {
            ty: 5,
            data: vec![1, 2],
        },
    )
    .unwrap();
    let expected = RegError::InvalidValueSize {
        ty: 5,
        expected: 4,
        actual: 2,
    };
    assert_eq!(h.get_value("", "bad"), Err(expected.clone()));
    assert_eq!(h.list_values(""), Err(expected));
}
#[test]
fn unicode_root_name_roundtrips() {
    let mut h = Hive::new_empty("根");
    let h = Hive::from_bytes(h.to_bytes().unwrap()).unwrap();
    assert_eq!(h.root_key().unwrap().name, "根");
}
#[test]
fn link_value_keeps_its_type() {
    let v = RegValue::from_raw(RegType::Link, &[65, 0, 0, 0]).unwrap();
    assert_eq!(v.reg_type(), regf_rs::RegType::Link);
}
fn big_data_hive() -> Vec<u8> {
    let mut h = Hive::new_empty("ROOT");
    h.set_value("", "segment1", RegValue::Binary(vec![0x41; 16344]))
        .unwrap();
    h.set_value("", "segment2", RegValue::Binary(vec![0x42; 10]))
        .unwrap();
    h.set_value("", "list", RegValue::Binary(vec![0; 8]))
        .unwrap();
    h.set_value("", "db", RegValue::Binary(vec![0; 8])).unwrap();
    h.set_value("", "big", RegValue::Binary(vec![0; 8]))
        .unwrap();
    let mut b = h.to_bytes().unwrap();
    let seg1 = rd(&b, vk(&b, 0) + 8);
    let seg2 = rd(&b, vk(&b, 1) + 8);
    let list = rd(&b, vk(&b, 2) + 8);
    let db = rd(&b, vk(&b, 3) + 8);
    let lp = 4096 + list as usize + 4;
    wr(&mut b, lp, seg1);
    wr(&mut b, lp + 4, seg2);
    let dp = 4096 + db as usize + 4;
    b[dp..dp + 4].copy_from_slice(&[b'd', b'b', 2, 0]);
    wr(&mut b, dp + 4, list);
    let vp = vk(&b, 4);
    wr(&mut b, vp + 4, 16354);
    wr(&mut b, vp + 8, db);
    b
}

#[test]
fn big_data_read_excludes_segment_alignment_padding() {
    let b = big_data_hive();
    let oracle = nt_hive::Hive::new(b.as_slice()).unwrap();
    let root = oracle.root_key_node().unwrap();
    let oracle_value = root
        .value("big")
        .unwrap()
        .unwrap()
        .data()
        .unwrap()
        .into_vec()
        .unwrap();
    assert_eq!(&oracle_value[16344..], &[0x42; 10]);
    let h = Hive::from_bytes(b).unwrap();
    let RegValue::Binary(value) = h.get_value("", "big").unwrap() else {
        panic!()
    };
    assert_eq!(&value[16344..], &[0x42; 10]);
}
#[test]
fn cyclic_subkey_index_returns_error() {
    let mut h = Hive::new_empty("ROOT");
    h.create_key("a").unwrap();
    let mut b = h.to_bytes().unwrap();
    let root = 4096 + rd(&b, 0x24) as usize + 4;
    let list = rd(&b, root + 28);
    let lp = 4096 + list as usize + 4;
    b[lp..lp + 2].copy_from_slice(b"ri");
    wr(&mut b, lp + 4, list);
    let mut h = Hive::from_bytes(b).unwrap();
    assert!(matches!(
        h.list_subkeys(""),
        Err(RegError::CorruptCell { .. })
    ));
    assert!(matches!(
        h.create_key("b"),
        Err(RegError::CorruptCell { .. })
    ));
}

#[test]
fn fixed_width_values_reject_short_and_long_data() {
    for (ty, expected) in [
        (RegType::Dword, 4),
        (RegType::DwordBigEndian, 4),
        (RegType::Qword, 8),
    ] {
        for actual in [0, expected - 1, expected + 1] {
            assert_eq!(
                RegValue::from_raw(ty, &vec![0; actual]),
                Err(RegError::InvalidValueSize {
                    ty: ty.to_u32(),
                    expected,
                    actual
                })
            );
        }
        assert!(RegValue::from_raw(ty, &vec![0; expected]).is_ok());
    }
}

#[test]
fn link_bytes_and_type_survive_hive_roundtrip() {
    // Preserve the original termination and even unpaired UTF-16 surrogates.
    for raw in [vec![65, 0], vec![65, 0, 0, 0], vec![0, 0xd8]] {
        let mut h = Hive::new_empty("ROOT");
        h.set_value("", "link", RegValue::Link(raw.clone()))
            .unwrap();
        let decoded = h.get_value("", "link").unwrap();
        h.set_value("", "link", decoded).unwrap();
        let bytes = h.to_bytes().unwrap();
        let oracle = nt_hive::Hive::new(bytes.as_slice()).unwrap();
        let root = oracle.root_key_node().unwrap();
        let v = root.value("link").unwrap().unwrap();
        assert_eq!(v.data_type().unwrap(), nt_hive::KeyValueDataType::RegLink);
        assert_eq!(v.data().unwrap().into_vec().unwrap(), raw);
    }
}

#[test]
fn root_name_encoding_matches_independent_reader() {
    for name in ["ROOT", "根", "é", "🦀"] {
        let mut h = Hive::new_empty(name);
        let bytes = h.to_bytes().unwrap();
        let oracle = nt_hive::Hive::new(bytes.as_slice()).unwrap();
        oracle.validate().unwrap();
        assert_eq!(
            oracle.root_key_node().unwrap().name().unwrap().to_string(),
            name
        );
    }
}

#[cfg(feature = "std")]
#[test]
fn saving_dirty_hive_preserves_destination() {
    let mut b = Hive::new_empty("ROOT").to_bytes().unwrap();
    let seq = rd(&b, 4);
    wr(&mut b, 8, seq + 1);
    let sum = Header::checksum(&b);
    wr(&mut b, 0x1fc, sum);
    let mut h = Hive::from_bytes(b).unwrap();
    let dir = std::env::temp_dir().join(format!(
        "regf-dirty-save-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("hive");
    std::fs::write(&path, b"original").unwrap();
    assert_eq!(
        h.save(&path).unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"original");
    assert!(h.is_dirty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn incomplete_big_data_is_rejected() {
    for segment_count in [1u16, 4] {
        let mut b = big_data_hive();
        let db = rd(&b, vk(&b, 4) + 8);
        let dp = 4096 + db as usize + 4;
        b[dp + 2..dp + 4].copy_from_slice(&segment_count.to_le_bytes());
        let h = Hive::from_bytes(b).unwrap();
        assert!(matches!(
            h.get_value("", "big"),
            Err(RegError::CorruptCell { .. })
        ));
    }
}

#[test]
fn short_big_data_segment_is_rejected() {
    let mut b = big_data_hive();
    let segment = rd(&b, vk(&b, 0) + 8);
    wr(&mut b, 4096 + segment as usize, (-8i32) as u32);
    let h = Hive::from_bytes(b).unwrap();
    assert!(matches!(
        h.get_value("", "big"),
        Err(RegError::CorruptCell { .. })
    ));
}

#[test]
fn truncated_external_and_oversized_inline_data_are_rejected() {
    for size in [0x80000005, 100] {
        let mut h = Hive::new_empty("ROOT");
        h.set_value("", "v", RegValue::Binary(vec![1; 8])).unwrap();
        let mut b = h.to_bytes().unwrap();
        let value = vk(&b, 0);
        wr(&mut b, value + 4, size);
        let h = Hive::from_bytes(b).unwrap();
        assert!(matches!(
            h.get_value("", "v"),
            Err(RegError::CorruptCell { .. })
        ));
    }
}

#[test]
fn header_finalization_rejects_dirty_state_without_mutating_bytes() {
    let mut b = Hive::new_empty("ROOT").to_bytes().unwrap();
    let mut header = Header::parse(&b).unwrap();
    header.secondary_sequence += 1;
    let before = b.clone();
    assert_eq!(header.finalize(&mut b), Err(RegError::DirtyHive));
    assert_eq!(b, before);
    assert!(header.is_dirty());
}

#[test]
fn hive_growth_starts_at_declared_bins_end_despite_file_padding() {
    for padding in [4096, 32768] {
        let mut bytes = Hive::new_empty("ROOT").to_bytes().unwrap();
        let old_end = 4096 + Header::parse(&bytes).unwrap().hive_bins_size as usize;
        bytes.resize(bytes.len() + padding, 0xa5);
        let mut hive = Hive::from_bytes(bytes).unwrap();
        hive.set_value("", "large", RegValue::Binary(vec![0x5a; 16344]))
            .unwrap();
        let bytes = hive.to_bytes().unwrap();
        assert_eq!(&bytes[old_end..old_end + 4], b"hbin");
        assert_eq!(rd(&bytes, old_end + 4) as usize, old_end - 4096);
        let end = 4096 + Header::parse(&bytes).unwrap().hive_bins_size as usize;
        let value = vk(&bytes, 0);
        let data = 4096 + rd(&bytes, value + 8) as usize;
        assert!(
            data + 4 + 16344 <= end,
            "value was written outside the declared hive bins"
        );
        let oracle = nt_hive::Hive::new(bytes.as_slice()).unwrap();
        oracle.validate().unwrap();
        let root = oracle.root_key_node().unwrap();
        assert_eq!(
            root.value("large")
                .unwrap()
                .unwrap()
                .data()
                .unwrap()
                .into_vec()
                .unwrap(),
            vec![0x5a; 16344]
        );
    }
}

#[test]
fn allocation_rejects_truncated_declared_bins() {
    let mut bytes = Hive::new_empty("ROOT").to_bytes().unwrap();
    let bins = rd(&bytes, 0x28);
    wr(&mut bytes, 0x28, bins + 4096);
    let checksum = Header::checksum(&bytes);
    wr(&mut bytes, 0x1fc, checksum);
    let mut hive = Hive::from_bytes(bytes).unwrap();
    assert!(matches!(
        hive.set_value("", "value", RegValue::Dword(1)),
        Err(RegError::Truncated { .. })
    ));
}

#[test]
fn allocation_rejects_a_bin_crossing_the_declared_region() {
    let mut bytes = Hive::new_empty("ROOT").to_bytes().unwrap();
    bytes.resize(bytes.len() + 4096, 0);
    wr(&mut bytes, 4096 + 8, 8192);
    let mut hive = Hive::from_bytes(bytes).unwrap();
    assert!(matches!(
        hive.set_value("", "value", RegValue::Dword(1)),
        Err(RegError::CorruptCell { .. })
    ));
}
