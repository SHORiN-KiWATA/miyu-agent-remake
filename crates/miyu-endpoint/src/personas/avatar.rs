//! 人格的头像（施工 P-5，`docs/blueprint/personas.md`「头像」，设计 `16-人格与预设.md` Y13）：一张 PNG、JPEG、WebP，存在人格
//! 目录里，照层叠走。`persona.set` 的 `avatar` 换、删管理员家目录那一层的；`persona.avatar` 读；`persona.list`、`persona.get`
//! 带它的版本，头照版本换缓存。

use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::ContentHash;
use miyu_store::personas::{AVATARS, Found, Personas, avatar_in};

use crate::Core;
use crate::refusal::Refusal;

/// 最大几个字节。
const MAX_BYTES: usize = 1024 * 1024;

/// 宽、高都不超过几个像素。
const MAX_SIDE: u32 = 1024;

/// 运行日志的目标。
const TARGET: &str = "miyu::personas";

/// `persona.set` 的 `avatar`：`blob`、`unset` 正好写一个；`expect` 是 `persona.get` 给的版本，`null` 是「你那一层还没有」。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AvatarParams {
    #[serde(default)]
    blob: Option<String>,
    #[serde(default)]
    unset: Option<bool>,
    #[serde(default, deserialize_with = "super::written")]
    expect: Option<Option<String>>,
}

/// 查过、读好了的一次换头像：要写的字节和扩展名（`None` 是删掉），对照的版本。
pub(crate) struct Checked {
    picture: Option<(Vec<u8>, &'static str)>,
    expect: Option<Option<String>>,
}

/// 查 `avatar`：blob 读出来照内容认，合规矩的才收。拒了的什么都不写（`persona.set` 先查它，再写别的）。
pub(crate) async fn check(core: &Core, params: AvatarParams) -> Result<Checked, Refusal> {
    let picture = match (params.blob, params.unset) {
        (Some(blob), None) => {
            let hash = ContentHash::parse(&blob).map_err(|_| Refusal::BAD_PARAMS)?;
            let (bytes, image) = crate::attach::picture(core, hash).await?;
            let Some((media_type, width, height)) = image else {
                return Err(Refusal::AVATAR_NOT_IMAGE);
            };
            let extension = match media_type.as_str() {
                "image/png" => "png",
                "image/jpeg" => "jpg",
                "image/webp" => "webp",
                _ => return Err(Refusal::AVATAR_NOT_IMAGE),
            };
            if bytes.len() > MAX_BYTES || width > MAX_SIDE || height > MAX_SIDE {
                return Err(Refusal::avatar_too_big(json!({
                    "bytes": bytes.len(),
                    "width": width,
                    "height": height,
                    "max_bytes": MAX_BYTES,
                    "max_side": MAX_SIDE,
                })));
            }
            Some((bytes, extension))
        }
        (None, Some(true)) => None,
        _ => return Err(Refusal::BAD_PARAMS),
    };
    Ok(Checked {
        picture,
        expect: params.expect,
    })
}

/// 把查过的写进人格 `id` 在管理员家目录那一层的目录：先删那一层原来的，再写新的（先写临时文件再改名）。`expect` 对不上的
/// `persona_conflict`，什么都不动。
pub(crate) fn apply(personas: &Personas, id: &str, checked: &Checked) -> Result<(), Refusal> {
    let dir = personas.home_dir(id);
    if let Some(expect) = &checked.expect {
        let current = avatar_in(&dir).as_deref().and_then(version_of);
        if &current != expect {
            return Err(Refusal::conflict("persona_conflict", Some(json!(current))));
        }
    }
    let failed = |error: std::io::Error| {
        tracing::warn!(target: TARGET, persona = id, kind = ?error.kind(), "persona avatar not written");
        Refusal::INTERNAL
    };
    if checked.picture.is_some() {
        std::fs::create_dir_all(&dir).map_err(failed)?;
    }
    for name in AVATARS {
        match std::fs::remove_file(dir.join(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(failed(error)),
        }
    }
    if let Some((bytes, extension)) = &checked.picture {
        let temporary = dir.join(format!(".avatar.{extension}.tmp"));
        std::fs::write(&temporary, bytes).map_err(failed)?;
        std::fs::rename(&temporary, dir.join(format!("avatar.{extension}"))).map_err(failed)?;
    }
    Ok(())
}

/// 头像的版本：图的字节的 SHA-256 前 16 位十六进制；读不了的没有。
fn version_of(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(ContentHash::of(&bytes).hex()[..16].to_string())
}

/// 叠好的人格的头像的版本：没有头像的是 `None`（`persona.list`、`persona.get` 的 `avatar`）。
pub(crate) fn version(found: &Found) -> Option<String> {
    found.avatar.as_deref().and_then(version_of)
}

/// `persona.avatar` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReadParams {
    persona: String,
}

/// `persona.avatar`：`{"avatar", "media_type", "data"}`；没有头像的回 `null`。
pub(crate) async fn read(core: &Core, params: ReadParams) -> Result<Value, Refusal> {
    let personas = super::personas(core);
    tokio::task::spawn_blocking(move || {
        let found = personas
            .find(&params.persona)
            .map_err(|error| super::told(&error, None))?;
        let Some(path) = found.avatar else {
            return Ok(Value::Null);
        };
        let bytes = std::fs::read(&path).map_err(|_| Refusal::INTERNAL)?;
        let media_type = match path.extension().and_then(|extension| extension.to_str()) {
            Some("png") => "image/png",
            Some("webp") => "image/webp",
            _ => "image/jpeg",
        };
        Ok(json!({
            "avatar": ContentHash::of(&bytes).hex()[..16].to_string(),
            "media_type": media_type,
            "data": STANDARD.encode(&bytes),
        }))
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?
}
