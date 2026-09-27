//! 工作目录太宽（施工 4-3 下）：家目录、根目录，包含数据根、落在数据根里而不在账号工作区里的，都太宽。

use std::path::{Path, PathBuf};

use miyu_fs::too_wide;

fn root() -> PathBuf {
    PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" })
}

#[test]
fn what_counts_as_too_wide() {
    let home = root().join("home").join("me");
    let data = home.join(".miyu");
    let own = data.join("home").join("admin").join("workspace");
    let wide = |dir: &Path| too_wide(dir, Some(&home), &data, &own);
    assert!(wide(&home), "家目录");
    assert!(wide(&root()), "根目录");
    assert!(wide(&root().join("home")), "包含数据根");
    assert!(wide(&data), "数据根本身");
    assert!(wide(&data.join("state")), "落在数据根里");
    assert!(!wide(&own), "账号自己的工作区");
    assert!(!wide(&own.join("sub")), "账号工作区里面");
    assert!(!wide(&home.join("proj")), "项目目录");
    // 家目录包含数据根：家目录读不出来也认得出太宽。
    assert!(too_wide(&home, None, &data, &own));
    // 数据根在别处（MIYU_HOME）：家目录只有读得出来才认得。
    let data = root().join("srv").join("miyu");
    let own = data.join("home").join("admin").join("workspace");
    assert!(too_wide(&home, Some(&home), &data, &own));
    assert!(!too_wide(&home, None, &data, &own));
}
