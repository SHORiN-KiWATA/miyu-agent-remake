use super::*;

/// 照第 2 版的格式拼一份记录：版本、大小、删的时间，字数（连结尾的 0），UTF-16 的路径。
fn second(path: &str, deleted: u64) -> Vec<u8> {
    let units: Vec<u16> = path.encode_utf16().chain([0]).collect();
    let mut record = Vec::new();
    record.extend(2u64.to_le_bytes());
    record.extend(4u64.to_le_bytes());
    record.extend(deleted.to_le_bytes());
    record.extend(u32::try_from(units.len()).expect("不长").to_le_bytes());
    record.extend(units.iter().flat_map(|unit| unit.to_le_bytes()));
    record
}

#[test]
fn the_second_version_gives_the_whole_path_and_when() {
    // CI 的 Windows 上真删出来的那一份：164 字节，路径 67 个字。
    let path = r"C:\Users\runneradmin\AppData\Local\Temp\miyu-base-8272-0\work\a.txt";
    let record = second(path, 0x01DD_4E7D_8B6D_D490);
    assert_eq!(record.len(), 164);
    assert_eq!(
        recorded(&record),
        Some(Recorded {
            path: path.to_string(),
            deleted: 0x01DD_4E7D_8B6D_D490,
        })
    );
}

#[test]
fn the_first_version_has_a_fixed_width_path() {
    let mut record = Vec::new();
    record.extend(1u64.to_le_bytes());
    record.extend(4u64.to_le_bytes());
    record.extend(7u64.to_le_bytes());
    let mut units: Vec<u16> = r"D:\old\b.txt".encode_utf16().collect();
    units.resize(260, 0);
    record.extend(units.iter().flat_map(|unit| unit.to_le_bytes()));
    assert_eq!(record.len(), 544);
    assert_eq!(
        recorded(&record),
        Some(Recorded {
            path: r"D:\old\b.txt".to_string(),
            deleted: 7,
        })
    );
}

#[test]
fn what_is_not_a_record_gives_nothing() {
    let good = second(r"C:\a.txt", 1);
    // 版本认不出的。
    let mut unknown = good.clone();
    unknown[0] = 3;
    assert_eq!(recorded(&unknown), None);
    // 字数说的比有的长。
    assert_eq!(recorded(&good[..good.len() - 2]), None);
    // 字数大得离谱：不溢出，没有。
    let mut huge = good.clone();
    huge[24..28].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(recorded(&huge), None);
    // 连头都不够的。
    assert_eq!(recorded(&good[..7]), None);
    assert_eq!(recorded(&good[..20]), None);
    // 第 1 版不够 544 字节的。
    let mut first = vec![0u8; 543];
    first[0] = 1;
    assert_eq!(recorded(&first), None);
}

#[test]
fn the_record_sits_next_to_what_was_kept() {
    let bin = Path::new("/bin/S-1-5-21");
    assert_eq!(
        record_of(&bin.join("$RVOYYZX.txt")),
        Some(bin.join("$IVOYYZX.txt"))
    );
    // 目录没有扩展名。
    assert_eq!(record_of(&bin.join("$RABC123")), Some(bin.join("$IABC123")));
    assert_eq!(record_of(&bin.join("a.txt")), None);
    assert_eq!(record_of(&bin.join("$IABC123")), None);
}
