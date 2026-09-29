//! 网络照常：连得上沙盒外起的 TCP 服务，听得了端口，`socketpair` 能用，解析得了名字，连得上 HTTPS。Unix 套接字照读写
//! 的规矩：能写的地方连得上，别处（Docker、ssh-agent 这类系统服务在的地方）和藏起来的连不上。

use std::net::TcpListener;
use std::os::unix::net::UnixListener;
use std::path::Path;

use super::*;
use crate::support::Dir;

#[test]
fn tcp_connections_and_listening_work() {
    let work = Dir::new();
    let cwd = real(work.path());
    let spec = spec(&[&cwd], &[]);
    let listener = TcpListener::bind("127.0.0.1:0").expect("听得了");
    let address = listener.local_addr().expect("有地址");
    let said = child(&spec, &cwd, &format!("tcp:{address}"));
    assert!(said.contains("child ok"), "连沙盒外的服务：{said}");
    let said = child(&spec, &cwd, "listen");
    assert!(said.contains("child ok"), "听端口：{said}");
}

/// 在 `dir` 里起一个听着的 Unix 套接字，交回它的真实位置和它自己（活着才连得上）。
fn listening(dir: &Path) -> (String, UnixListener) {
    let path = real(dir).join("s.sock");
    let listener = UnixListener::bind(&path).expect("听得了");
    (path.to_str().expect("路径是 UTF-8").to_string(), listener)
}

#[test]
fn unix_sockets_connect_only_inside_writable_places() {
    let work = Dir::new();
    let elsewhere = Dir::new();
    let base = Dir::new();
    let data = real(base.path()).join("data");
    std::fs::create_dir(&data).expect("建得了目录");
    let cwd = real(work.path());
    let spec = spec(&[&cwd, &real(base.path())], &[&data]);
    let (inside, _inside) = listening(&cwd);
    let (outside, _outside) = listening(elsewhere.path());
    let (hidden, _hidden) = listening(&data);
    let said = child(&spec, &cwd, &format!("unix:{inside}"));
    assert!(said.contains("child ok"), "能写的地方：{said}");
    for path in [outside, hidden] {
        let said = child(&spec, &cwd, &format!("unix:{path}"));
        assert!(
            said.contains("child failed: Operation not permitted"),
            "{path}：{said}"
        );
    }
    let said = child(&spec, &cwd, "pair");
    assert!(said.contains("child ok"), "socketpair：{said}");
}

#[test]
fn names_resolve_and_https_works() {
    let work = Dir::new();
    let cwd = real(work.path());
    let spec = spec(&[&cwd], &[]);
    // 解析名字要连 mDNSResponder 的 Unix 套接字：底子单独放行了它。
    let said = child(&spec, &cwd, "resolve:localhost");
    assert!(said.contains("child ok"), "{said}");
    // 系统的 HTTPS（NSURLSession，`nscurl` 用的就是它）经 trustd.agent 验证书：底子放行了它。`curl` 自己带证书，
    // 测不到这一条。要上网：CI 上连 github.com。
    let page = cwd.join("page");
    let out = run(
        &spec,
        &cwd,
        "/usr/bin/nscurl",
        &[
            "-o",
            page.to_str().expect("路径是 UTF-8"),
            "https://github.com",
        ],
    );
    let size = std::fs::metadata(&page).map_or(0, |meta| meta.len());
    assert!(size > 0, "没下载下来：{out:?}");
}
