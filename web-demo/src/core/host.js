// @ts-check
//! 旧代码（`src/ui/`、`src/markdown/`）经这里用宿主（蓝图 `web/architecture.md`「宿主」）：本机文件、blob、链接卡片配图的
//! 地址，外面的链接，剪贴板。内核起来时把选好的宿主交进来；软件包不经这里，经服务 `host`。拆完旧代码这个文件就没了。

/** @type {import('../host/browser.js').Host|null} */
let host = null;

/** 内核起来时交进来。 */
export function useHost(h) {
  host = h;
}

/**
 * 一个本机文件的地址。`download` 为真的叫浏览器存下来。宿主还没交进来时是空的。能不能读照核心（`fs.read`），不分会话。
 * @param {string} path 绝对路径
 */
export function fileUrl(path, download = false) {
  return host ? host.urls.file(path, download) : '';
}

/**
 * 一个 blob 的地址（你的话里的附件）。`download` 为真的叫浏览器存下来，存成 `name`。照这个账号的 blob（`blob.get`），不分会话。
 * @param {string} hash `sha256:…`
 * @param {string} type 媒体类型
 * @param {{download?: boolean, name?: string|null}} [how]
 */
export function blobUrl(hash, type, how = {}) {
  return host ? host.urls.blob(hash, type, how) : '';
}


/** 外面的链接：浏览器是新标签页，桌面端是系统的浏览器。 */
export function openExternal(url) {
  host?.open(url);
}

/** 写进剪贴板。 */
export function writeClipboard(text) {
  if (!host) return Promise.reject(new Error('host'));
  return host.clipboard.write(text);
}
