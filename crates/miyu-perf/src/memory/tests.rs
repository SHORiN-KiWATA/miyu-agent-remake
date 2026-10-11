//! 读 `smaps_rollup`、`children` 的字；Linux 上量得到自己。

use super::*;

/// 一份真的 `smaps_rollup`（Linux 6.x 的写法）。
const ROLLUP: &str = "\
55d0c0a3e000-7ffd7b3f9000 ---p 00000000 00:00 0                          [rollup]
Rss:               12948 kB
Pss:                9101 kB
Pss_Dirty:          5524 kB
Pss_Anon:           5480 kB
Pss_File:           3621 kB
Pss_Shmem:             0 kB
Shared_Clean:       5368 kB
Shared_Dirty:          0 kB
Private_Clean:      2056 kB
Private_Dirty:      5524 kB
Referenced:        12948 kB
Anonymous:          5480 kB
KSM:                   0 kB
LazyFree:              0 kB
AnonHugePages:         0 kB
";

#[test]
fn the_rollup_is_read() {
    assert_eq!(
        parse_rollup(ROLLUP),
        Some(Usage {
            rss: 12948,
            pss: 9101,
            anon: 5480,
        })
    );
    // `Pss_Anon` 不能当成 `Pss`，少了一项的读不成。
    let without_pss: String = ROLLUP
        .lines()
        .filter(|line| !line.starts_with("Pss:"))
        .map(|line| format!("{line}\n"))
        .collect();
    assert_eq!(parse_rollup(&without_pss), None);
}

#[test]
fn children_are_read() {
    assert_eq!(parse_children("123 456 \n"), vec![123, 456]);
    assert_eq!(parse_children(""), Vec::<u32>::new());
}

#[cfg(target_os = "linux")]
#[test]
fn it_measures_itself_and_a_child() {
    let mut child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .unwrap();
    // `spawn` 走 vfork：内核换掉内存时就放父进程走，进程名稍后才换成 `sleep`（`begin_new_exec` 里 `exec_mmap` 在
    // `__set_task_comm` 前头），这一小会儿读到的还是测试线程的名字。等它换过来，最多五秒（CI 上偶发过两回）。
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let found = loop {
        let found = tree(std::process::id());
        let named = found
            .iter()
            .any(|process| process.pid == child.id() && process.name == "sleep");
        if named || std::time::Instant::now() > deadline {
            break found;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    child.kill().unwrap();
    child.wait().unwrap();
    assert_eq!(found[0].pid, std::process::id());
    assert!(found[0].usage.pss > 0);
    assert!(
        found
            .iter()
            .any(|process| process.pid == child.id() && process.name == "sleep")
    );
}
