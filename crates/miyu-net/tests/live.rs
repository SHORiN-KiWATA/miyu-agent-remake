//! 真网络的实测（施工单 W-7 再补「验收」第 3 条）：照出厂的规矩、平时的闸和环境变量里的代理，去抓真的站，把卡片
//! 印出来给人看。要连外网，结果跟着对方的站变，所以两道锁：标了 `#[ignore]`，平时的 `cargo test` 不跑；CI 的长跑
//! 那一项用 `--ignored` 跑所有标了的测试，所以还要设 `MIYU_NET_LIVE=1` 才真去连，没设的什么都不做。在能上网的
//! 机器上这样跑：
//!
//! ```text
//! MIYU_NET_LIVE=1 cargo test -p miyu-net --test live -- --ignored --nocapture
//! ```
//!
//! 图存进一个用完就删的临时目录，不碰真实数据。

use std::time::Duration;

use crate::support::{Store, resources};
use miyu_net::{Kind, LinkPreview, Preview};

/// 要试的链接和应该是什么：（链接，卡片的 `kind`）。
const LINKS: &[(&str, Kind)] = &[
    ("https://www.bilibili.com/video/BV1Rxam6kEtU", Kind::Video),
    ("https://www.youtube.com/watch?v=dQw4w9WgXcQ", Kind::Video),
    (
        "https://en.wikipedia.org/wiki/Rust_(programming_language)",
        Kind::Article,
    ),
    ("https://zh.wikipedia.org/wiki/Rust", Kind::Article),
    ("https://wiki.archlinux.org/title/Pacman", Kind::Article),
    ("https://github.com/rust-lang/rust", Kind::Page),
];

#[tokio::test]
#[ignore = "要连外网：设 MIYU_NET_LIVE=1 在能上网的机器上手动跑"]
async fn real_sites_make_real_cards() {
    if std::env::var_os("MIYU_NET_LIVE").is_none() {
        println!("没设 MIYU_NET_LIVE=1：不连外网，跳过");
        return;
    }
    let store = Store::new();
    let links = LinkPreview::new(&resources(), store.blobs.clone());
    let mut wrong = Vec::new();
    for (url, kind) in LINKS {
        let got = tokio::time::timeout(Duration::from_secs(60), links.preview(url))
            .await
            .unwrap_or_else(|_| panic!("一分钟内没抓完 {url}"))
            .expect("link_preview.json 读得懂");
        println!("\n== {url}");
        match got {
            Preview::Card(card) => {
                println!("  kind:        {}", card.kind.as_str());
                println!("  title:       {}", card.title);
                println!("  description: {}", card.description);
                println!("  site:        {}", card.site);
                println!("  author:      {:?}", card.author);
                println!("  duration:    {:?}", card.duration);
                println!(
                    "  image:       {:?}",
                    card.image.map(|image| image.media_type)
                );
                println!("  icon:        {:?}", card.icon.map(|icon| icon.media_type));
                println!("  url:         {}", card.url);
                if card.kind != *kind || card.description.is_empty() {
                    wrong.push(format!(
                        "{url}：kind 是 {:?}，简介 {:?}",
                        card.kind, card.description
                    ));
                }
            }
            Preview::Miss(why) => {
                println!("  没有卡片：{}", why.as_str());
                wrong.push(format!("{url}：没有卡片（{}）", why.as_str()));
            }
        }
    }
    assert!(wrong.is_empty(), "这几条不像样：\n{}", wrong.join("\n"));
}
