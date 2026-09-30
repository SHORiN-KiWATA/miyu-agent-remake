// @ts-check
//! 浏览器这个宿主（蓝图 `web/architecture.md`「宿主」，照 Linux 的 `arch/`）：和平台有关的都在这一份里，页面、内核、软件包
//! 不直接碰（测试照源码查）。以后桌面端（Tauri）加一份 `tauri.js`，交出一样的东西，别处不动。
//!
//! 现在经桥（`web-demo/bridge/`）：
//! - 访问口令：桥起来时打出的网址带着 `#k=…`；拿到以后从地址栏抹掉，记在这个标签页的 sessionStorage 里，刷新还能连
//!   （设计 21 X6：放在 `#` 后面，不发给服务器、不进 Referer）。口令不出这个文件。
//! - 连核心的线是一条 WebSocket（`/ws`）；本机文件、blob、链接卡片配图的地址是桥的 `/file`、`/blob`、`/link-image`；
//!   附件先交给桥（`/upload`，蓝图 `web.md`「附件」第 2 条）。
//! - 链接照网页的写法（`target=_blank`、`download`），浏览器自己会办，`intercept` 什么都不做。

/**
 * @typedef {{name: string, size: number, type: string, file?: Blob, path?: string}} FileRef
 *   一个要当附件的文件：浏览器给的只有内容（`file`），桌面端给的有路径（`path`）
 * @typedef {{readyState: number, send: (text: string) => void, onopen: any, onmessage: any, onclose: any, onerror: any}} Channel
 *   连核心的一条线，样子照 WebSocket：`readyState` 是 1 时通着，一帧一条 JSON-RPC 消息
 * @typedef {{getItem: (k: string) => string|null, setItem: (k: string, v: string) => void, removeItem: (k: string) => void}} Store
 *   这台设备上存东西的存法
 */

const KEY = 'miyu.web.bridge-key';

/** 这个标签页有没有桥的口令：地址里带着的先记下、从地址栏抹掉；没带的用记着的。都没有是 `null`。 */
function bridgeKey() {
  const m = location.hash.match(/k=([0-9a-f]+)/);
  if (m) {
    try { sessionStorage.setItem(KEY, m[1]); } catch { /* 记不住就只管这一次 */ }
    history.replaceState(null, '', location.pathname + location.search);
    return m[1];
  }
  try { return sessionStorage.getItem(KEY); } catch { return null; }
}

/**
 * 桥给的几种地址，都带口令。
 * @param {string} key 访问口令
 */
export function urls(key) {
  return {
    /** 一个本机文件：`download` 为真的叫浏览器存下来。 */
    file: (/** @type {string} */ session, /** @type {string} */ path, download = false) => {
      const q = new URLSearchParams({ k: key, session, path });
      if (download) q.set('download', '1');
      return `/file?${q}`;
    },
    /** 一个 blob（你的话里的附件）：下载的存成 `name`。 */
    blob: (/** @type {string} */ session, /** @type {string} */ hash, /** @type {string} */ type, /** @type {{download?: boolean, name?: string|null}} */ how = {}) => {
      const q = new URLSearchParams({ k: key, session, hash, type });
      if (how.download) q.set('download', '1');
      if (how.download && how.name) q.set('name', how.name);
      return `/blob?${q}`;
    },
    /** 链接卡片的配图、图标：桥抓来记在内存里的，照编号取。 */
    linkImage: (/** @type {string} */ id) => `/link-image?${new URLSearchParams({ k: key, id })}`,
  };
}

/** 浏览器的文件换成附件用的样子。 */
const refs = (/** @type {Iterable<File>} */ list) => [...list].map((file) => /** @type {FileRef} */ ({ name: file.name, size: file.size, type: file.type, file }));

/** 选文件：系统的选文件窗口，能多选；取消的交回空的。 */
function pick() {
  return new Promise((resolve) => {
    const input = /** @type {HTMLInputElement} */ (document.createElement('input'));
    input.type = 'file';
    input.multiple = true;
    input.addEventListener('change', () => resolve(refs(input.files ?? [])), { once: true });
    input.addEventListener('cancel', () => resolve([]), { once: true });
    input.click();
  });
}

/**
 * 拖文件（蓝图 `web.md`「附件」第 1 条）：文件一拖进窗口 `enter`，到了 `target` 上面、离开它 `over(真假)`，拖出窗口、松开了
 * `leave`；只有在 `target` 上松开才 `drop`（交回文件；附件给的是整页，在哪松开都收）。在别处松开什么都不做，页面也不跳去打开那个文件。拖的不是文件的不管。
 * 交回怎么不看。桌面端照窗口的拖放事件和落点做同一件事。
 * @param {HTMLElement} target 收文件的那一块
 * @param {{enter: () => void, over: (inside: boolean) => void, leave: () => void, drop: (files: FileRef[]) => void}} on
 */
function watchDrop(target, on) {
  let depth = 0;
  let inside = false;
  const isFiles = (/** @type {DragEvent} */ e) => !!e.dataTransfer && [...e.dataTransfer.types].includes('Files');
  const within = (/** @type {DragEvent} */ e) => e.target instanceof Node && target.contains(e.target);
  const hover = (/** @type {boolean} */ yes) => {
    if (yes === inside) return;
    inside = yes;
    on.over(yes);
  };
  const end = () => {
    depth = 0;
    hover(false);
    on.leave();
  };
  /** @type {[string, (e: DragEvent) => void][]} */
  const events = [
    ['dragenter', (e) => {
      if (!isFiles(e)) return;
      e.preventDefault();
      if (depth++ === 0) on.enter();
      hover(within(e));
    }],
    ['dragover', (e) => {
      if (!isFiles(e)) return;
      e.preventDefault();
      const yes = within(e);
      if (e.dataTransfer) e.dataTransfer.dropEffect = yes ? 'copy' : 'none';
      hover(yes);
    }],
    ['dragleave', (e) => {
      if (!isFiles(e) || --depth > 0) return;
      end();
    }],
    ['drop', (e) => {
      if (!isFiles(e)) return;
      e.preventDefault();
      const yes = within(e);
      end();
      if (yes) on.drop(refs(e.dataTransfer?.files ?? []));
    }],
  ];
  for (const [name, fn] of events) document.addEventListener(name, /** @type {any} */ (fn));
  return () => { for (const [name, fn] of events) document.removeEventListener(name, /** @type {any} */ (fn)); };
}

/**
 * 框里的缩略图：图片、视频交回一个临时地址和怎么松开它；别的是 `null`。
 * @param {FileRef} ref
 */
function preview(ref) {
  if (!/^(image|video)\//.test(ref.type) || !(ref.file instanceof Blob)) return null;
  const url = URL.createObjectURL(ref.file);
  return { url, release: () => URL.revokeObjectURL(url) };
}

/**
 * 读文字文件的内容（框里的卡写有几行）：超过 `max` 字节的、读不了的交 `null`。
 * @param {FileRef} ref
 * @param {number} max
 */
async function text(ref, max) {
  if (!(ref.file instanceof Blob) || ref.size > max) return null;
  try { return await ref.file.text(); } catch { return null; }
}

/**
 * 附件变成核心读得到的路径：有路径的就是它；只有内容的交给桥存进临时目录。桥不收的抛出它的原话。
 * @param {string} key
 * @param {FileRef} ref
 * @returns {Promise<string>}
 */
async function stage(key, ref) {
  if (ref.path) return ref.path;
  const res = await fetch(`/upload?${new URLSearchParams({ k: key, name: ref.name })}`, { method: 'POST', body: ref.file });
  if (!res.ok) throw new Error((await res.text()) || res.statusText);
  return (await res.json()).path;
}

/** 这台设备上存东西：`localStorage`（隐私窗口、清过数据时读写会抛，由内核的 `storage` 兜着）。 */
const store = /** @type {Store} */ ({
  getItem: (k) => localStorage.getItem(k),
  setItem: (k, v) => localStorage.setItem(k, v),
  removeItem: (k) => localStorage.removeItem(k),
});

/** 起浏览器这个宿主；链接里、这个标签页里都没有桥的口令的交 `null`（页面说连不上桥）。 */
export function browserHost() {
  const key = bridgeKey();
  if (!key) return null;
  return {
    kind: 'browser',
    /** 开一条到核心的线（经桥）。 */
    channel: () => /** @type {Channel} */ (/** @type {unknown} */ (new WebSocket(`${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/ws?k=${key}`))),
    urls: urls(key),
    files: { pick, watchDrop, preview, refs, text, stage: (/** @type {FileRef} */ ref) => stage(key, ref) },
    /** 外面的链接：新标签页。 */
    open: (/** @type {string} */ url) => { window.open(url, '_blank', 'noopener'); },
    /** 接住页面里的链接：浏览器自己会办，什么都不做；交回怎么不接。 */
    intercept: (/** @type {HTMLElement} */ root) => () => {},
    clipboard: { write: (/** @type {string} */ text) => navigator.clipboard.writeText(text) },
    store,
  };
}

/** @typedef {NonNullable<ReturnType<typeof browserHost>>} Host 一个宿主交出的（桌面端那一份照这个样子） */
