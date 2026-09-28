//! 平常的工具照常干活：照核心会写的那种规格（工作区、临时目录能写，数据根藏起来），跑 `/bin/sh`、`shell` 用的
//! `zsh`、`git`（经 Xcode 的转接）、Rust 的工具链、C 编译器。

use std::path::PathBuf;

use super::*;
use crate::support::Dir;

#[test]
fn everyday_tools_work_under_a_realistic_spec() {
    let base = Dir::new();
    let root = real(base.path());
    let workspace = root.join("ws");
    let data = root.join("data");
    std::fs::create_dir_all(&workspace).expect("建得了目录");
    std::fs::create_dir_all(&data).expect("建得了目录");
    let temp = real(&std::env::temp_dir());
    let spec = spec(&[&workspace, &temp], &[&data]);
    assert_eq!(ok(&sh(&spec, &workspace, "echo sh-ok")), "sh-ok\n");
    assert_eq!(
        ok(&run(
            &spec,
            &workspace,
            "/bin/zsh",
            &["-f", "+o", "nomatch", "-c", "print zsh-ok; pwd -P"]
        )),
        format!("zsh-ok\n{}\n", workspace.display())
    );
    let git = "git init -q && git -c user.name=t -c user.email=t@t commit -q --allow-empty -m x && git log --oneline | wc -l";
    assert_eq!(ok(&sh(&spec, &workspace, git)).trim(), "1");
    let cargo = PathBuf::from(std::env::var_os("CARGO").expect("cargo 跑的测试有 CARGO"));
    let rustc = cargo.with_file_name("rustc");
    for tool in [&cargo, &rustc] {
        let said = ok(&run(
            &spec,
            &workspace,
            tool.to_str().expect("路径是 UTF-8"),
            &["--version"],
        ));
        assert!(said.contains("1."), "{said}");
    }
    let cc = "printf 'int main(void){return 0;}' > a.c && cc -o a a.c && ./a && echo cc-ok";
    assert_eq!(ok(&sh(&spec, &workspace, cc)), "cc-ok\n");
}
