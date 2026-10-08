//! `miyu-web serve`（施工 W-9，`web-module.md`「怎么走」第九条第 1 到 6 款）：单实例、`run/web`、那一行；端口被占说清楚；
//! Host 只认三种写法；页面文件不出页面目录；响应头一个不少、从不设 cookie；空闲到点退出、删 `run/web`、放锁。

use crate::support::*;
use miyu_ipc::Ready;
use miyu_web::serve::{address, running};

#[tokio::test]
async fn one_instance_writes_its_address_and_a_second_says_running() {
    let home = Home::new();
    let (ready, port, _first) = start(&home, 0, 600).await;
    assert_eq!(ready, Ready::Ready);
    assert!(port > 0);
    assert_eq!(
        address(&home.root),
        Some(format!("http://127.0.0.1:{port}"))
    );
    assert!(running(&home.root));
    let (again, _, second) = start(&home, 0, 600).await;
    assert_eq!(again, Ready::Running);
    assert_eq!(within("第二个走", second).await.expect("没崩"), Ok(()));
}

#[tokio::test]
async fn a_taken_port_is_said_plainly() {
    let home = Home::new();
    let taken = std::net::TcpListener::bind("127.0.0.1:0").expect("占得了");
    let port = taken.local_addr().expect("有地址").port();
    let (ready, _, running) = start(&home, port, 600).await;
    assert_eq!(ready, Ready::Failed(format!("port {port} in use")));
    assert!(within("走", running).await.expect("没崩").is_err());
    assert_eq!(address(&home.root), None, "没写地址");
}

#[tokio::test]
async fn pages_stay_inside_and_carry_the_headers() {
    let home = Home::new();
    home.page("index.html", "<h1>miyu</h1>");
    home.page("app.js", "let x = 1;");
    home.page("sub/style.css", "p{}");
    home.page("data.unknown", "?");
    std::fs::write(home.dir.join("secret.txt"), "secret").expect("写得进");
    #[cfg(unix)]
    std::os::unix::fs::symlink(home.dir.join("secret.txt"), home.pages.join("link.txt"))
        .expect("建得了");
    #[cfg(unix)]
    assert!(
        std::process::Command::new("mkfifo")
            .arg(home.pages.join("pipe"))
            .status()
            .expect("有 mkfifo")
            .success()
    );
    let (_, port, _serving) = start(&home, 0, 600).await;
    let host = format!("127.0.0.1:{port}");
    let index = request(port, "GET", "/", &host, &[]).await;
    assert_eq!(index.status, 200);
    assert_eq!(index.body, b"<h1>miyu</h1>");
    assert_eq!(
        index.header("content-type"),
        Some("text/html; charset=utf-8")
    );
    assert_eq!(index.header("x-content-type-options"), Some("nosniff"));
    assert_eq!(index.header("referrer-policy"), Some("no-referrer"));
    assert_eq!(index.header("cache-control"), Some("no-cache"));
    assert_eq!(
        index.header("content-security-policy"),
        Some("default-src 'self'; connect-src 'self'; frame-ancestors 'none'")
    );
    assert_eq!(index.header("set-cookie"), None, "从不设 cookie");
    let script = request(port, "GET", "/app.js", &host, &[]).await;
    assert_eq!(
        script.header("content-type"),
        Some("text/javascript; charset=utf-8")
    );
    assert_eq!(
        request(port, "GET", "/sub/style.css", &host, &[])
            .await
            .status,
        200
    );
    // 表里没有的扩展名：不猜，`application/octet-stream`（再带 `nosniff`，浏览器不当页面跑）。
    let unknown = request(port, "GET", "/data.unknown", &host, &[]).await;
    assert_eq!(
        unknown.header("content-type"),
        Some("application/octet-stream")
    );
    let head = request(port, "HEAD", "/app.js", &host, &[]).await;
    assert_eq!((head.status, head.body.len()), (200, 0));
    for outside in [
        "/../secret.txt",
        "/%2e%2e/secret.txt",
        "/sub/../../secret.txt",
        // 带 `..` 的一律不要，绕回页面目录里的也不要。
        "/sub/../app.js",
        "/sub",
        "/nope.js",
        "/link.txt",
        "/%zz",
    ] {
        let answer = request(port, "GET", outside, &host, &[]).await;
        assert_eq!(answer.status, 404, "{outside}");
        assert_eq!(answer.header("x-content-type-options"), Some("nosniff"));
    }
    // 不是普通文件的不读：读一个命名管道会一直等。等不到回应的，替它写一头关掉，测试照样结束。
    #[cfg(unix)]
    {
        let pipe = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            request(port, "GET", "/pipe", &host, &[]),
        )
        .await;
        let Ok(pipe) = pipe else {
            drop(std::fs::File::create(home.pages.join("pipe")));
            panic!("读了命名管道");
        };
        assert_eq!(pipe.status, 404);
    }
    assert_eq!(request(port, "POST", "/", &host, &[]).await.status, 405);
    // Host 只认三种写法：别的网站把域名解析到回环地址也进不来。
    for good in [
        format!("localhost:{port}"),
        format!("LOCALHOST:{port}"),
        format!("[::1]:{port}"),
    ] {
        assert_eq!(
            request(port, "GET", "/", &good, &[]).await.status,
            200,
            "{good}"
        );
    }
    for bad in [
        format!("evil.example:{port}"),
        "127.0.0.1".to_string(),
        format!("127.0.0.1:{}", port + 1),
    ] {
        assert_eq!(
            request(port, "GET", "/", &bad, &[]).await.status,
            403,
            "{bad}"
        );
    }
}

#[tokio::test]
async fn it_leaves_when_idle_and_cleans_up() {
    let home = Home::new();
    let (_, port, serving) = start(&home, 0, 1).await;
    // 有人来要页面不算忙：只数 WebSocket。
    let _ = request(port, "GET", "/", &format!("127.0.0.1:{port}"), &[]).await;
    let ran = within("空闲退出", serving).await.expect("没崩");
    assert_eq!(ran, Ok(()));
    assert_eq!(address(&home.root), None, "删了 run/web");
    assert!(!running(&home.root), "放了锁");
}
