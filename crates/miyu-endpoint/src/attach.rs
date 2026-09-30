//! 附件（施工 3-9 三补，`docs/blueprint/protocol.md` 的 `blob.put`、`session.send`；`04-核心协议.md` 第九节）：人说话时
//! 附上的图片、文件。
//!
//! - [`put`]：`blob.put`，本机的头传路径、核心自己读，远程的头传内容；认是什么（[`mod@kind`]），存成管理员的 blob，回应
//!   给头看的几样。
//! - [`blocks`]：`session.send` 的 `attachments` 变成内容块：blob 要在，照内容再认一遍，块里的宽高、媒体类型都是核心
//!   自己量的。
//!
//! 读文件、读 blob、存 blob 都碰磁盘，在阻塞线程里做。

mod kind;

use std::io::Read;
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde_json::{Value, json};

use miyu_fs::{Boundary, Places, Zone, open_file, resolve, tilde};
use miyu_kernel::block::{Block, File, Image};
use miyu_kernel::id::{AccountId, ContentHash, FileName, MediaType};
use miyu_store::blob::{BlobError, Blobs};
use miyu_store::root::DataRoot;

use crate::Core;
use crate::refusal::Refusal;
use kind::{Kind, kind};

/// 一个附件最多几个字节：20 MiB（`04-核心协议.md` 第十一节；分块上传以后再说）。
pub(crate) const LIMIT: u64 = 20 * 1024 * 1024;

/// 运行日志的来源。
const TARGET: &str = "miyu::endpoint";

/// `blob.put` 的参数：`path`、`data` 正好写一个。
#[derive(Debug, Deserialize)]
pub(crate) struct PutParams {
    /// 本机的文件：绝对路径，或者 `~` 开头的。
    #[serde(default)]
    path: Option<String>,
    /// 文件的内容，base64。
    #[serde(default)]
    data: Option<String>,
    /// 文件名；不写的取路径的最后一段，传内容的必写。
    #[serde(default)]
    name: Option<String>,
    /// 媒体类型；不写的照内容认。
    #[serde(default)]
    media_type: Option<String>,
}

/// `session.send`、`session.redo` 的一个附件：`blob.put` 的回应，只看这三格。
#[derive(Debug, Deserialize)]
pub(crate) struct Attachment {
    blob: String,
    name: String,
    media_type: String,
}

/// 从哪来：本机的路径（文件名可以没写），或者头交来的内容和文件名。
enum Source {
    Path(String, Option<FileName>),
    Data(Vec<u8>, FileName),
}

/// 读、存要的几样：数据根、管理员、系统的家目录。
struct Place {
    root: DataRoot,
    admin: AccountId,
    home: Option<PathBuf>,
}

/// `blob.put`：读、认、存，回应 `blob`、`name`、`media_type`、`kind`，图片另带 `width`、`height`。
pub(crate) async fn put(core: &Core, params: PutParams) -> Result<Value, Refusal> {
    let name = params.name.as_deref().map(file_name).transpose()?;
    let given = params.media_type.as_deref().map(media_type).transpose()?;
    let source = match (params.path, params.data, name) {
        (Some(path), None, name) if Path::new(&path).is_absolute() || tilde(&path).is_some() => {
            Source::Path(path, name)
        }
        (None, Some(data), Some(name)) => Source::Data(
            STANDARD.decode(data).map_err(|_| Refusal::BAD_PARAMS)?,
            name,
        ),
        _ => return Err(Refusal::BAD_PARAMS),
    };
    let place = place(core);
    blocking(move || put_blocking(&place, source, given)).await
}

/// `session.send`、`session.redo`（施工 4-7 再补）的附件变成内容块，照先后。
pub(crate) async fn blocks(
    core: &Core,
    attachments: Vec<Attachment>,
) -> Result<Vec<Block>, Refusal> {
    let mut wanted = Vec::with_capacity(attachments.len());
    for attachment in attachments {
        let blob = ContentHash::parse(&attachment.blob).map_err(|_| Refusal::BAD_PARAMS)?;
        wanted.push((
            blob,
            file_name(&attachment.name)?,
            media_type(&attachment.media_type)?,
        ));
    }
    if wanted.is_empty() {
        return Ok(Vec::new());
    }
    let place = place(core);
    blocking(move || {
        let blobs = Blobs::new(place.root.blobs(&place.admin));
        wanted
            .into_iter()
            .map(|(blob, name, given)| block(&blobs, blob, name, given))
            .collect()
    })
    .await
}

/// 一个附件造成一块：blob 要在，照内容再认一遍。
fn block(
    blobs: &Blobs,
    blob: ContentHash,
    name: FileName,
    given: MediaType,
) -> Result<Block, Refusal> {
    let bytes = match blobs.get(&blob) {
        Ok(bytes) => bytes,
        Err(BlobError::Missing(_)) => return Err(Refusal::UNKNOWN_ATTACHMENT),
        Err(error) => {
            tracing::warn!(target: TARGET, blob = blob.as_str(), error = %error, "attachment not read");
            return Err(Refusal::INTERNAL);
        }
    };
    Ok(
        match kind(&bytes, Some(given)).map_err(|_| Refusal::ATTACHMENT_TOO_BIG)? {
            Kind::Image {
                media_type,
                width,
                height,
            } => Block::Image(Image {
                blob,
                media_type,
                width,
                height,
            }),
            Kind::File { media_type } => Block::File(File {
                blob,
                name,
                media_type,
            }),
        },
    )
}

/// 在阻塞线程里：读来源、认、存。
fn put_blocking(place: &Place, source: Source, given: Option<MediaType>) -> Result<Value, Refusal> {
    let (bytes, name) = match source {
        Source::Data(bytes, name) => (bytes, name),
        Source::Path(path, name) => {
            let (bytes, real) = read(place, &path)?;
            let name = match name {
                Some(name) => name,
                None => last_segment(&path, &real)?,
            };
            (bytes, name)
        }
    };
    if bytes.len() as u64 > LIMIT {
        return Err(Refusal::ATTACHMENT_TOO_BIG);
    }
    let found = kind(&bytes, given).map_err(|_| Refusal::ATTACHMENT_TOO_BIG)?;
    let blob = Blobs::new(place.root.blobs(&place.admin))
        .put(&bytes)
        .map_err(|error| {
            tracing::warn!(target: TARGET, error = %error, "attachment not stored");
            Refusal::INTERNAL
        })?;
    Ok(match found {
        Kind::Image {
            media_type,
            width,
            height,
        } => json!({
            "blob": blob.as_str(), "name": name.as_str(), "media_type": media_type.as_str(),
            "kind": "image", "width": width, "height": height,
        }),
        Kind::File { media_type } => json!({
            "blob": blob.as_str(), "name": name.as_str(), "media_type": media_type.as_str(),
            "kind": "file",
        }),
    })
}

/// 读本机的一个文件，交回内容和它真实的位置：换成真实的位置，数据根里（管理员的工作区以外）的不给，路上一层链接都
/// 不跟地打开，最多读到上限多一个字节。
fn read(place: &Place, path: &str) -> Result<(Vec<u8>, PathBuf), Refusal> {
    let real = resolve(Path::new("/"), place.home.as_deref(), path)
        .map_err(|_| Refusal::ATTACHMENT_UNREADABLE)?;
    let places = Places::here(
        place.root.workspace(&place.admin),
        place.root.path().to_path_buf(),
        place.home.as_deref(),
    );
    if Boundary::new(&places).zone(&real) == Zone::Forbidden {
        return Err(Refusal::ATTACHMENT_IN_DATA_ROOT);
    }
    let file = open_file(&real).map_err(|_| Refusal::ATTACHMENT_UNREADABLE)?;
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Refusal::ATTACHMENT_UNREADABLE)?;
    Ok((bytes, real))
}

/// 没写文件名的，取头写的路径的最后一段；没有的（例如以 `..` 结尾），取真实位置的最后一段。
fn last_segment(path: &str, real: &Path) -> Result<FileName, Refusal> {
    let segment = Path::new(path)
        .file_name()
        .or_else(|| real.file_name())
        .ok_or(Refusal::BAD_PARAMS)?;
    file_name(&segment.to_string_lossy())
}

fn file_name(text: &str) -> Result<FileName, Refusal> {
    FileName::parse(text).map_err(|_| Refusal::BAD_PARAMS)
}

fn media_type(text: &str) -> Result<MediaType, Refusal> {
    MediaType::parse(text).map_err(|_| Refusal::BAD_PARAMS)
}

fn place(core: &Core) -> Place {
    Place {
        root: core.root.clone(),
        admin: core.admin.clone(),
        home: core.home.clone(),
    }
}

/// 在阻塞线程里跑；崩了的是核心的问题。
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, Refusal> + Send + 'static,
) -> Result<T, Refusal> {
    tokio::task::spawn_blocking(work)
        .await
        .unwrap_or_else(|error| {
            tracing::error!(target: TARGET, error = %error, "attachment panicked");
            Err(Refusal::INTERNAL)
        })
}
