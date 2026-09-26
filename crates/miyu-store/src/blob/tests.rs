//! blob 的测试：存了再取；放在哪；同一份存两遍；崩溃留下的临时文件；撞名；改名时目标已经有了；
//! 读出来不对；两个账号各存各的。

use std::time::Duration;

use miyu_kernel::id::AccountId;

use super::*;
use crate::env::{Env, Platform};
use crate::root::DataRoot;
use crate::test_support::Scratch;

fn blobs_in(scratch: &Scratch) -> Blobs {
    Blobs::new(scratch.path().join("blobs"))
}

#[test]
fn what_is_put_comes_back() {
    let scratch = Scratch::new();
    let blobs = blobs_in(&scratch);
    // 二进制：带 \0、\r\n，存进去什么，取出来就是什么。
    let content = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\xff".to_vec();
    let hash = blobs.put(&content).unwrap();
    assert_eq!(hash, ContentHash::of(&content));
    assert_eq!(blobs.get(&hash).unwrap(), content);
    assert_eq!(fs::read(blobs.path(&hash)).unwrap(), content);
    // 存完，临时文件都改了名，tmp/ 是空的。
    let tmp = scratch.path().join("blobs").join("tmp");
    assert_eq!(fs::read_dir(tmp).unwrap().count(), 0);
}

#[test]
fn a_blob_lives_under_its_first_two_hex_digits() {
    // 路径写死，不照代码算：「abc」的 SHA-256（FIPS 180-2 的测试值），放在 <前两位>/<64 位>，
    // 文件名里没有 sha256: 的冒号。
    let scratch = Scratch::new();
    let blobs = blobs_in(&scratch);
    let hash = blobs.put(b"abc").unwrap();
    let hex = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    let path = scratch.path().join("blobs").join("ba").join(hex);
    assert_eq!(blobs.path(&hash), path);
    assert_eq!(fs::read(&path).unwrap(), b"abc");
}

#[test]
fn the_same_content_is_stored_once_and_freshened() {
    let scratch = Scratch::new();
    let blobs = blobs_in(&scratch);
    let hash = blobs.put(b"same").unwrap();
    let path = blobs.path(&hash);
    // 先把修改时间设到很久以前，像一个快过宽限期的。
    let long_ago = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
    OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(long_ago)
        .unwrap();
    assert_eq!(blobs.put(b"same").unwrap(), hash);
    // 修改时间回到了现在附近；不比精确的值，三个平台的精度不一样。
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    let age = SystemTime::now()
        .duration_since(modified)
        .unwrap_or_default();
    assert!(
        age < Duration::from_secs(3600),
        "没有刷成现在：{modified:?}"
    );
    // 只有一个文件。
    assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    assert_eq!(blobs.get(&hash).unwrap(), b"same");
}

#[test]
fn a_crash_before_the_rename_leaves_only_a_temp_file() {
    // 崩在改名之前：tmp/ 里有一个写了一半的，最终的名字上什么都没有。
    let scratch = Scratch::new();
    let blobs = blobs_in(&scratch);
    let content = b"the whole picture";
    let tmp = scratch.path().join("blobs").join("tmp");
    fs::create_dir_all(&tmp).unwrap();
    fs::write(tmp.join("1-0"), &content[..5]).unwrap();
    let hash = ContentHash::of(content);
    assert!(matches!(blobs.get(&hash), Err(BlobError::Missing(missing)) if missing == hash));
    // 再存一遍，取得到完整的。
    assert_eq!(blobs.put(content).unwrap(), hash);
    assert_eq!(blobs.get(&hash).unwrap(), content);
    // 崩溃留下的那个，回收的时候再清，现在不碰。
    assert_eq!(fs::read(tmp.join("1-0")).unwrap(), &content[..5]);
}

#[test]
fn a_taken_temp_name_is_skipped() {
    let scratch = Scratch::new();
    let dir = scratch.path();
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("a"), "崩溃留下的").unwrap();
    fs::write(dir.join("b"), "崩溃留下的").unwrap();
    let mut names = ["a", "b", "c"].into_iter().map(String::from);
    let (path, file) = create_temp(dir, || names.next().unwrap()).unwrap();
    drop(file);
    assert_eq!(path, dir.join("c"));
    assert_eq!(fs::read_to_string(dir.join("a")).unwrap(), "崩溃留下的");
    assert_eq!(fs::read_to_string(dir.join("b")).unwrap(), "崩溃留下的");
}

#[test]
fn settling_onto_a_blob_that_is_already_there_is_fine() {
    // 两个会话同时存同一份：改名的时候，目标已经有了。
    let scratch = Scratch::new();
    let dir = scratch.path();
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("blob"), "内容").unwrap();
    fs::write(dir.join("temp"), "内容").unwrap();
    settle(&dir.join("temp"), &dir.join("blob")).unwrap();
    assert!(!dir.join("temp").exists());
    assert_eq!(fs::read_to_string(dir.join("blob")).unwrap(), "内容");
    // 改名失败了（这里是临时文件已经没了），可目标在：也算成功。
    settle(&dir.join("gone"), &dir.join("blob")).unwrap();
    // 目标也不在：照实报错。
    assert!(settle(&dir.join("gone"), &dir.join("nothing")).is_err());
}

#[test]
fn a_blob_that_does_not_match_its_name_is_reported() {
    let scratch = Scratch::new();
    let blobs = blobs_in(&scratch);
    let hash = blobs.put(b"original").unwrap();
    let path = blobs.path(&hash);
    fs::write(&path, "tampered").unwrap();
    let error = blobs.get(&hash).unwrap_err();
    assert!(matches!(&error, BlobError::Corrupt(corrupt) if *corrupt == hash));
    // 报错写明是哪一个；不自动修，也不删。
    assert!(error.to_string().contains(hash.as_str()), "{error}");
    assert_eq!(fs::read_to_string(&path).unwrap(), "tampered");
}

#[test]
fn each_account_keeps_its_own() {
    let scratch = Scratch::new();
    let root = DataRoot::locate(&Env {
        platform: Platform::current(),
        miyu_home: Some(scratch.path().join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
    })
    .unwrap();
    let alice = AccountId::parse("alice").unwrap();
    let bob = AccountId::parse("bob").unwrap();
    assert_eq!(
        root.blobs(&alice),
        scratch
            .path()
            .join("data")
            .join("home")
            .join("alice")
            .join("blobs")
    );
    // 同一份内容，两个账号各存一份（S5：不跨账号去重）。
    let alices = Blobs::new(root.blobs(&alice));
    let bobs = Blobs::new(root.blobs(&bob));
    let hash = alices.put(b"same picture").unwrap();
    assert_eq!(bobs.put(b"same picture").unwrap(), hash);
    assert_ne!(alices.path(&hash), bobs.path(&hash));
    assert!(alices.path(&hash).is_file());
    assert!(bobs.path(&hash).is_file());
}
