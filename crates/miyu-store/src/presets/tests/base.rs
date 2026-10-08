//! 以谁为底（施工 P-3 上，16 第四节）：自己的几层逐格盖在底叠好的样子上，底也照这样找；绕成圈、指着没有的、底写错了的报错。

use super::*;

#[test]
fn own_layers_go_over_what_the_base_stacks_to() {
    let places = Places::new();
    places.write(Layer::Shipped, "dev.toml", DEV);
    places.write(Layer::System, "dev.toml", "[software]\ngoal = true\n");
    places.write(
        Layer::Home,
        "mine.toml",
        "[preset]\nbase = \"dev\"\nname = { zh = \"我的\" }\n\n[software]\nmemory = true\nnet = false\n",
    );
    let found = places.presets.find("mine").unwrap();
    assert_eq!(found.base.as_deref(), Some("dev"));
    assert_eq!(found.layers, [Layer::Home], "底的几层不算在自己的里");
    let file = &found.file;
    assert_eq!(
        (
            file.name.get("zh").map(String::as_str),
            file.name.get("en").map(String::as_str)
        ),
        (Some("我的"), Some("Dev")),
        "名字逐种语言盖"
    );
    assert_eq!(file.default_persona.as_deref(), Some("engineer"));
    assert_eq!(file.unlisted(), Unlisted::Off, "底的系统区那一层也叠进来");
    assert_eq!(
        (
            file.opens("basesystem"),
            file.opens("goal"),
            file.opens("memory"),
            file.opens("net"),
        ),
        (true, true, true, false)
    );
}

#[test]
fn a_base_of_a_base_is_found_the_same_way() {
    let places = Places::new();
    places.write(Layer::Shipped, "dev.toml", DEV);
    places.write(
        Layer::Home,
        "middle.toml",
        "[preset]\nbase = \"dev\"\n\n[tools]\nshell = false\n",
    );
    places.write(Layer::Home, "top.toml", "[preset]\nbase = \"middle\"\n");
    let found = places.presets.find("top").unwrap();
    assert_eq!(found.base.as_deref(), Some("middle"), "写的是直接的底");
    assert!(found.file.tools_off.contains("shell"));
    assert_eq!(found.file.default_persona.as_deref(), Some("engineer"));
    let plain = places.presets.find("dev").unwrap();
    assert_eq!(plain.base, None);
    // 指纹不算 `base`：没写底的预设和 P-3 以前算的一个字节不差，以前造的快照照旧对得上（这个值是 P-3 以前的代码算的）。
    assert_eq!(
        plain.file.digest().as_str(),
        "sha256:fdfb3980af10583089985762e272fb7eff179e587e72ad9e328169723512b5aa",
        "没有底的指纹和以前一样"
    );
}

#[test]
fn cycles_missing_bases_and_broken_bases_are_errors() {
    let places = Places::new();
    places.write(Layer::Home, "a.toml", "[preset]\nbase = \"b\"\n");
    places.write(Layer::Home, "b.toml", "[preset]\nbase = \"a\"\n");
    places.write(Layer::Home, "me.toml", "[preset]\nbase = \"me\"\n");
    places.write(Layer::Home, "lost.toml", "[preset]\nbase = \"nowhere\"\n");
    places.write(
        Layer::Home,
        "on-broken.toml",
        "[preset]\nbase = \"broken\"\n",
    );
    places.write(Layer::System, "broken.toml", "[preset]\nnope = 1\n");
    places.write(Layer::Home, "bad.toml", "[preset]\nbase = \"Not An Id\"\n");
    let error = |id: &str| places.presets.find(id).unwrap_err().to_string();
    assert_eq!(error("a"), "base cycle: a -> b -> a");
    assert_eq!(error("me"), "base cycle: me -> me");
    assert_eq!(error("lost"), r#"base "nowhere" of "lost" not found"#);
    assert_eq!(
        error("on-broken"),
        "system broken.toml:2: unknown key preset.nope"
    );
    let PresetError::Invalid(_, _, problem) = places.presets.find("bad").unwrap_err() else {
        panic!("写错了的底是文件写错");
    };
    assert_eq!(problem.code, Code::BadBase);
    assert_eq!(problem.line, Some(2));
}
