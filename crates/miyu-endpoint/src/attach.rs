//! 附件（施工 3-9 三补，`docs/blueprint/protocol.md` 的 `blob.put`、`session.send`；`04-核心协议.md` 第九节）：人说话时
//! 附上的图片、文件。
//!
//! - [`put`]：`blob.put`，本机的头传路径、核心自己读，远程的头传内容；认是什么（[`mod@kind`]），存成管理员的 blob，回应
//!   给头看的几样。
//! - [`blocks`]：`session.send` 的 `attachments` 变成内容块：blob 要在，照内容再认一遍，块里的宽高、媒体类型都是核心
//!   自己量的；图片块、文件块都带着头交回来的名字（图片的施工 3-9 四补）。
//! - [`images`]：`model.call` 的图照哈希变成图片块（施工 8-20）：blob 要在这个账号里，照内容认，不是图的参数不对；不带
//!   名字。
//! - [`get`]：`blob.get`，分块读这个账号的一个 blob，照属主给，不照会话（施工 W-6，`web-module.md`「七、分块读」）。
//!
//! 读文件、读 blob、存 blob 都碰磁盘，在阻塞线程里做。
//!
//! 认是什么（[`mod@kind`]）、文件名和媒体类型怎么查（[`file_name`]、[`media_type`]）、存好了怎么拼回应
//! （[`reply`]）、一个最多几个字节（[`LIMIT`]）：这几样和分块上传（`uploads.rs`，施工 W-5）共用一份，不重写
//! 一遍（`web-module.md`「在哪」）。

pub(crate) mod kind;

use std::io::Read;
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde_json::{Value, json};

use miyu_fs::{Boundary, Places, Zone, open_file, resolve, tilde};
use miyu_kernel::block::{Block, File, Image};
use miyu_kernel::id::{AccountId, ContentHash, FileName, MediaType, SourcePath};
use miyu_store::blob::{BlobError, Blobs};
use miyu_store::root::DataRoot;

use crate::Core;
use crate::refusal::Refusal;
use kind::{Kind, kind};

/// 一个附件最多几个字节：20 MiB（`04-核心协议.md` 第十一节）。`blob.open` 的 `size` 也照这个数（施工 W-5，
/// `uploads.rs`）。
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

/// `session.send`、`session.redo` 的一个附件：`blob.put` 的回应，只看这三格；另可带原来的路径（施工 3-9 五补）。
#[derive(Debug, Deserialize)]
pub(crate) struct Attachment {
    blob: String,
    name: String,
    media_type: String,
    /// 原来在本机的哪儿：头 `blob.put` 传的那个路径。只查写法（[`SourcePath`]），不碰磁盘。
    #[serde(default)]
    path: Option<String>,
}

/// `blob.get` 的参数（施工 W-6）：`blob` 必写（内容哈希）；`offset` 不写是 0；`length` 不写是
/// [`miyu_fs::MAX_LENGTH`]，最多这个数，写 0 只问大小。
#[derive(Debug, Deserialize)]
pub(crate) struct GetParams {
    blob: String,
    #[serde(default)]
    offset: u64,
    #[serde(default = "default_length")]
    length: u64,
}

/// `blob.get` 不写 `length` 时读多少（施工 W-6，`fs.read` 同样的默认值在 `files.rs`）。
fn default_length() -> u64 {
    miyu_fs::MAX_LENGTH
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
        let path = attachment
            .path
            .as_deref()
            .map(SourcePath::parse)
            .transpose()
            .map_err(|_| Refusal::BAD_PARAMS)?;
        wanted.push((
            blob,
            file_name(&attachment.name)?,
            media_type(&attachment.media_type)?,
            path,
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
            .map(|(blob, name, given, path)| block(&blobs, blob, name, given, path))
            .collect()
    })
    .await
}

/// 附件存在连接的账号（现在都是管理员）名下，她照会话的属主 `owner` 读（施工 O-4 下）：不是一个账号的，把块里的 blob 拷一份
/// 进属主的，属主那里已经有的不再拷。读不了、存不下的记一行、`internal_error`。
pub(crate) async fn hand_over(
    core: &Core,
    owner: &AccountId,
    blocks: &[Block],
) -> Result<(), Refusal> {
    let hashes = blobs_of(blocks);
    if *owner == core.admin || hashes.is_empty() {
        return Ok(());
    }
    let place = place(core);
    let owner = owner.clone();
    blocking(move || copy_over(&place.root, &place.admin, &owner, &hashes)).await
}

/// 块里引用的 blob：图片、文件的，照先后。
pub(crate) fn blobs_of(blocks: &[Block]) -> Vec<ContentHash> {
    blocks
        .iter()
        .filter_map(|block| match block {
            Block::Image(image) => Some(image.blob.clone()),
            Block::File(file) => Some(file.blob.clone()),
            _ => None,
        })
        .collect()
}

/// 把 `hashes` 从管理员 `admin` 名下拷进 `owner` 名下（施工 O-4 下；扩展工具交回的图也走它，施工 O-2 三补）：属主那里已经有的
/// 不再拷，所以属主就是管理员的只看在不在。管理员名下也没有的 `unknown_attachment`；读不了、存不下的记一行、`internal_error`。
pub(crate) fn copy_over(
    root: &DataRoot,
    admin: &AccountId,
    owner: &AccountId,
    hashes: &[ContentHash],
) -> Result<(), Refusal> {
    let from = Blobs::new(root.blobs(admin));
    let to = Blobs::new(root.blobs(owner));
    for hash in hashes {
        if to.path(hash).is_file() {
            continue;
        }
        let bytes = read_blob(&from, hash)?;
        to.put(&bytes).map_err(|error| {
            tracing::warn!(target: TARGET, blob = hash.as_str(), error = %error, "attachment not handed over");
            Refusal::INTERNAL
        })?;
    }
    Ok(())
}

/// `model.call` 的图（施工 8-20）：每个哈希一块图片，照先后。blob 这个账号没有的 `unknown_attachment`，不是图的
/// `bad_params`，太大的 `attachment_too_big`。
pub(crate) async fn images(core: &Core, hashes: Vec<ContentHash>) -> Result<Vec<Block>, Refusal> {
    if hashes.is_empty() {
        return Ok(Vec::new());
    }
    let place = place(core);
    blocking(move || {
        let blobs = Blobs::new(place.root.blobs(&place.admin));
        hashes
            .into_iter()
            .map(|blob| {
                let bytes = read_blob(&blobs, &blob)?;
                match kind(&bytes, None).map_err(|_| Refusal::ATTACHMENT_TOO_BIG)? {
                    Kind::Image {
                        media_type,
                        width,
                        height,
                    } => Ok(Block::Image(Image {
                        blob,
                        name: None,
                        media_type,
                        width,
                        height,
                        path: None,
                    })),
                    Kind::File { .. } => Err(Refusal::BAD_PARAMS),
                }
            })
            .collect()
    })
    .await
}

/// 管理员的一个 blob 读出来、照内容认（施工 P-5：人格的头像）：交回字节和认出来的图；不是图、超了图的上限的交回 `None` 的图。
/// blob 没有的 `unknown_attachment`。
pub(crate) async fn picture(
    core: &Core,
    blob: ContentHash,
) -> Result<(Vec<u8>, Option<(MediaType, u32, u32)>), Refusal> {
    let place = place(core);
    blocking(move || {
        let bytes = read_blob(&Blobs::new(place.root.blobs(&place.admin)), &blob)?;
        let image = match kind(&bytes, None) {
            Ok(Kind::Image {
                media_type,
                width,
                height,
            }) => Some((media_type, width, height)),
            Ok(Kind::File { .. }) | Err(_) => None,
        };
        Ok((bytes, image))
    })
    .await
}

/// 取一个 blob：没有的 `unknown_attachment`，读不了的记一行、`internal_error`。
fn read_blob(blobs: &Blobs, blob: &ContentHash) -> Result<Vec<u8>, Refusal> {
    blobs.get(blob).map_err(|error| match error {
        BlobError::Missing(_) => Refusal::UNKNOWN_ATTACHMENT,
        error => {
            tracing::warn!(target: TARGET, blob = blob.as_str(), error = %error, "attachment not read");
            Refusal::INTERNAL
        }
    })
}

/// `blob.get`：分块读这个账号的一个 blob，照属主给，不照会话（施工 W-6，`web-module.md`「七、分块读」第 1、12
/// 条）。没有这个 blob：`unknown_blob`；`length` 超过 [`miyu_fs::MAX_LENGTH`]：`bad_params`。
pub(crate) async fn get(core: &Core, params: GetParams) -> Result<Value, Refusal> {
    if params.length > miyu_fs::MAX_LENGTH {
        return Err(Refusal::BAD_PARAMS);
    }
    let hash = ContentHash::parse(&params.blob).map_err(|_| Refusal::BAD_PARAMS)?;
    let place = place(core);
    blocking(move || get_blocking(&place, &hash, params.offset, params.length)).await
}

fn get_blocking(
    place: &Place,
    hash: &ContentHash,
    offset: u64,
    length: u64,
) -> Result<Value, Refusal> {
    let blobs = Blobs::new(place.root.blobs(&place.admin));
    let (data, size) = blobs
        .read_range(hash, offset, length)
        .map_err(|error| match error {
            BlobError::Missing(_) => Refusal::UNKNOWN_BLOB,
            error => {
                tracing::warn!(target: TARGET, error = %error, "blob not read");
                Refusal::INTERNAL
            }
        })?;
    Ok(json!({"data": STANDARD.encode(&data), "size": size}))
}

/// 一个附件造成一块：blob 要在，照内容再认一遍；名字照头交回来的，图片也带（施工 3-9 四补）；原来的路径照头交回来的
/// （施工 3-9 五补）。
fn block(
    blobs: &Blobs,
    blob: ContentHash,
    name: FileName,
    given: MediaType,
    path: Option<SourcePath>,
) -> Result<Block, Refusal> {
    let bytes = read_blob(blobs, &blob)?;
    Ok(
        match kind(&bytes, Some(given)).map_err(|_| Refusal::ATTACHMENT_TOO_BIG)? {
            Kind::Image {
                media_type,
                width,
                height,
            } => Block::Image(Image {
                blob,
                name: Some(name),
                media_type,
                width,
                height,
                path,
            }),
            Kind::File { media_type } => Block::File(File {
                blob,
                name,
                media_type,
                path,
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
    Ok(reply(&blob, &name, found))
}

/// 存好了，拼回应：`blob`、`name`、`media_type`、`kind`，图片另带 `width`、`height`（第 5 条）。分块上传
/// `blob.close` 共用这一份（施工 W-5）。
pub(crate) fn reply(blob: &ContentHash, name: &FileName, found: Kind) -> Value {
    match found {
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
    }
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

/// 文件名合不合写法（`kernel/ids.md`）。分块上传 `blob.open` 共用这一份（施工 W-5）。
pub(crate) fn file_name(text: &str) -> Result<FileName, Refusal> {
    FileName::parse(text).map_err(|_| Refusal::BAD_PARAMS)
}

/// 媒体类型合不合写法（`kernel/ids.md`）。分块上传 `blob.open` 共用这一份（施工 W-5）。
pub(crate) fn media_type(text: &str) -> Result<MediaType, Refusal> {
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
