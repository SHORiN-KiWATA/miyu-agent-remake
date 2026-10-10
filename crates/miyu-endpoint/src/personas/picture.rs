//! 人格的图：头像（施工 P-5，`docs/blueprint/personas.md`「头像」，设计 `16-人格与预设.md` Y13）和背景图（施工 P-6，「主题色、
//! 背景图」，Y15）。一张 PNG、JPEG、WebP，存在人格目录里，照层叠走。`persona.set` 的 `avatar`、`background` 换、删管理员家目录
//! 那一层的；`persona.avatar`、`persona.background` 读；`persona.list`、`persona.get` 带它们的版本，头照版本换缓存。两种只差
//! 名字和上限（[`Picture`]）。

use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::ContentHash;
use miyu_store::personas::{AVATARS, BACKGROUNDS, Found, Personas, avatar_in, background_in};

use crate::Core;
use crate::refusal::Refusal;

/// 运行日志的目标。
const TARGET: &str = "miyu::personas";

/// 人格的哪一种图。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Picture {
    /// 头像：最大 1 MiB，宽、高都不超过 1024。
    Avatar,
    /// 背景图：最大 5 MiB（`blob.put` 收图的上限），宽、高都不超过 4096。
    Background,
}

impl Picture {
    /// 协议里的名字：`persona.set` 的那一格、`persona.get` 的版本、读回来的那一格，也是文件名的开头。
    fn field(self) -> &'static str {
        match self {
            Picture::Avatar => "avatar",
            Picture::Background => "background",
        }
    }

    /// 同一层里认的文件名，照先后挑。
    fn names(self) -> [&'static str; 3] {
        match self {
            Picture::Avatar => AVATARS,
            Picture::Background => BACKGROUNDS,
        }
    }

    /// 最大几个字节。
    fn max_bytes(self) -> usize {
        match self {
            Picture::Avatar => 1024 * 1024,
            Picture::Background => 5 * 1024 * 1024,
        }
    }

    /// 宽、高都不超过几个像素。
    fn max_side(self) -> u32 {
        match self {
            Picture::Avatar => 1024,
            Picture::Background => 4096,
        }
    }

    /// 不是这三种图。
    fn not_image(self) -> Refusal {
        match self {
            Picture::Avatar => Refusal::AVATAR_NOT_IMAGE,
            Picture::Background => Refusal::BACKGROUND_NOT_IMAGE,
        }
    }

    /// 太大、太宽太高：`data` 是量到的和上限。
    fn too_big(self, measured: Value) -> Refusal {
        match self {
            Picture::Avatar => Refusal::avatar_too_big(measured),
            Picture::Background => Refusal::background_too_big(measured),
        }
    }

    /// 人格目录 `dir` 里的这一种图。
    fn in_dir(self, dir: &Path) -> Option<std::path::PathBuf> {
        match self {
            Picture::Avatar => avatar_in(dir),
            Picture::Background => background_in(dir),
        }
    }

    /// 叠好的人格的这一种图。
    fn of(self, found: &Found) -> Option<&Path> {
        match self {
            Picture::Avatar => found.avatar.as_deref(),
            Picture::Background => found.background.as_deref(),
        }
    }
}

/// `persona.set` 的 `avatar`、`background`：`blob`、`unset` 正好写一个；`expect` 是 `persona.get` 给的版本，`null` 是「你那一层
/// 还没有」。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PictureParams {
    #[serde(default)]
    blob: Option<String>,
    #[serde(default)]
    unset: Option<bool>,
    #[serde(default, deserialize_with = "super::written")]
    expect: Option<Option<String>>,
}

/// 查过、读好了的一次换图：哪一种，要写的字节和扩展名（`None` 是删掉），对照的版本。
pub(crate) struct Checked {
    picture: Picture,
    bytes: Option<(Vec<u8>, &'static str)>,
    expect: Option<Option<String>>,
}

/// 查 `picture` 那一格：blob 读出来照内容认，合规矩的才收。拒了的什么都不写（`persona.set` 先查它，再写别的）。
pub(crate) async fn check(
    core: &Core,
    picture: Picture,
    params: PictureParams,
) -> Result<Checked, Refusal> {
    let bytes = match (params.blob, params.unset) {
        (Some(blob), None) => {
            let hash = ContentHash::parse(&blob).map_err(|_| Refusal::BAD_PARAMS)?;
            let (bytes, image) = crate::attach::picture(core, hash).await?;
            let Some((media_type, width, height)) = image else {
                return Err(picture.not_image());
            };
            let extension = match media_type.as_str() {
                "image/png" => "png",
                "image/jpeg" => "jpg",
                "image/webp" => "webp",
                _ => return Err(picture.not_image()),
            };
            let (max_bytes, max_side) = (picture.max_bytes(), picture.max_side());
            if bytes.len() > max_bytes || width > max_side || height > max_side {
                return Err(picture.too_big(json!({
                    "bytes": bytes.len(),
                    "width": width,
                    "height": height,
                    "max_bytes": max_bytes,
                    "max_side": max_side,
                })));
            }
            Some((bytes, extension))
        }
        (None, Some(true)) => None,
        _ => return Err(Refusal::BAD_PARAMS),
    };
    Ok(Checked {
        picture,
        bytes,
        expect: params.expect,
    })
}

/// 把查过的写进人格 `id` 在管理员家目录那一层的目录：先删那一层原来的，再写新的（先写临时文件再改名）。`expect` 对不上的
/// `persona_conflict`，什么都不动。
pub(crate) fn apply(personas: &Personas, id: &str, checked: &Checked) -> Result<(), Refusal> {
    let picture = checked.picture;
    let dir = personas.home_dir(id);
    if let Some(expect) = &checked.expect {
        let current = picture.in_dir(&dir).as_deref().and_then(version_of);
        if &current != expect {
            return Err(Refusal::conflict("persona_conflict", Some(json!(current))));
        }
    }
    let failed = |error: std::io::Error| {
        tracing::warn!(target: TARGET, persona = id, picture = picture.field(), kind = ?error.kind(), "persona picture not written");
        Refusal::INTERNAL
    };
    if checked.bytes.is_some() {
        std::fs::create_dir_all(&dir).map_err(failed)?;
    }
    for name in picture.names() {
        match std::fs::remove_file(dir.join(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(failed(error)),
        }
    }
    if let Some((bytes, extension)) = &checked.bytes {
        let field = picture.field();
        let temporary = dir.join(format!(".{field}.{extension}.tmp"));
        std::fs::write(&temporary, bytes).map_err(failed)?;
        std::fs::rename(&temporary, dir.join(format!("{field}.{extension}"))).map_err(failed)?;
    }
    Ok(())
}

/// 图的版本：字节的 SHA-256 前 16 位十六进制；读不了的没有。
fn version_of(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(ContentHash::of(&bytes).hex()[..16].to_string())
}

/// 叠好的人格的这一种图的版本：没有的是 `None`（`persona.list`、`persona.get` 的 `avatar`、`background`）。
pub(crate) fn version(found: &Found, picture: Picture) -> Option<String> {
    picture.of(found).and_then(version_of)
}

/// `persona.avatar`、`persona.background` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReadParams {
    persona: String,
}

/// `persona.avatar`、`persona.background`：`{"<avatar|background>": <版本>, "media_type", "data"}`；没有的回 `null`。
pub(crate) async fn read(
    core: &Core,
    picture: Picture,
    params: ReadParams,
) -> Result<Value, Refusal> {
    let personas = super::personas(core);
    tokio::task::spawn_blocking(move || {
        let found = personas
            .find(&params.persona)
            .map_err(|error| super::told(&error, None))?;
        let Some(path) = picture.of(&found) else {
            return Ok(Value::Null);
        };
        let bytes = std::fs::read(path).map_err(|_| Refusal::INTERNAL)?;
        let media_type = match path.extension().and_then(|extension| extension.to_str()) {
            Some("png") => "image/png",
            Some("webp") => "image/webp",
            _ => "image/jpeg",
        };
        Ok(json!({
            picture.field(): ContentHash::of(&bytes).hex()[..16].to_string(),
            "media_type": media_type,
            "data": STANDARD.encode(&bytes),
        }))
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?
}
