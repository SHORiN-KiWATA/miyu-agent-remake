// @ts-check
//! 附件的纯逻辑（软件包 `attachments`，蓝图 `web.md`「附件」）：收哪些、框里那一排每一块在什么状态、发的时候交出什么、
//! 核心拒了放回来。不碰 DOM、不碰网络，画和传在 `tray.js`、`index.js`。


/**
 * @typedef {{name: string, size: number, type: string, file?: Blob, path?: string, stored?: {session: string, hash: string}}} FileLike
 *   宿主给的一个文件（`host/browser.js` 的 `FileRef`；测试里是假的）；输入历史翻出来的是核心存好的那一份（`stored`：发在哪个会话、编号）
 * @typedef {{blob: string, name: string, media_type: string, kind?: string, width?: number, height?: number}} Put `blob.put` 的回应
 * @typedef {{id: number, file: FileLike, name: string, size: number, state: 'uploading'|'ready', put: Put|null, recalled?: boolean}} Item
 *   框里的一块；`recalled` 是跟着输入历史翻出来、还没留下的（换一条时换掉）
 * @typedef {{blob: string, name: string, media_type: string, size: number}} Kept 记进输入历史的一块：核心存好的那一份
 * @typedef {{attachments: {blob: string, name: string, media_type: string}[]}} Taken 发的时候交出去的，合进 `session.send` 的参数
 */

/** 媒体类型的写法（`kernel/ids.md` 第 5 条）：两截，只用小写字母、数字和 `!#$&^_.+-`，最长 127 个字符。 */
const MEDIA_TYPE = /^[a-z0-9!#$&^_.+-]+\/[a-z0-9!#$&^_.+-]+$/;
const MEDIA_TYPE_MAX = 127;

/**
 * 浏览器给的媒体类型：合规矩的照写（换成小写），空的、不合的是 `null`，不写，交给核心照内容认（第 2 条）。
 * @param {string} type
 */
export function mediaType(type) {
  const t = (type ?? '').toLowerCase();
  return t.length <= MEDIA_TYPE_MAX && MEDIA_TYPE.test(t) ? t : null;
}

/**
 * 这一回放进来的几个里收哪些：超过 `max_mib` 的不收；框里已经有 `count` 个，放满 `max_files` 个以后的不收（第 1、2 条）。
 * @template {FileLike} F
 * @param {number} count 框里已经有几个
 * @param {F[]} files
 * @param {{max_files: number, max_mib: number}} config
 */
export function admit(count, files, config) {
  const limit = config.max_mib * 1024 * 1024;
  const fits = files.filter((f) => f.size <= limit);
  const room = Math.max(0, config.max_files - count);
  return {
    accepted: fits.slice(0, room),
    tooBig: files.filter((f) => f.size > limit),
    tooMany: Math.max(0, fits.length - room),
  };
}

/**
 * 一个文件是哪一种：框里的样子、图标照它（第 3 条）。先看媒体类型，没有的看扩展名。
 * @param {{name: string, type: string}} file
 * @returns {'image'|'video'|'audio'|'pdf'|'text'|'file'}
 */
export function kindOf(file) {
  const type = (file.type ?? '').toLowerCase();
  const ext = file.name.includes('.') ? file.name.slice(file.name.lastIndexOf('.') + 1).toLowerCase() : '';
  if (type.startsWith('image/')) return 'image';
  if (type.startsWith('video/')) return 'video';
  if (type.startsWith('audio/')) return 'audio';
  if (type === 'application/pdf' || ext === 'pdf') return 'pdf';
  if (type.startsWith('text/') || ['md', 'txt', 'json', 'csv', 'log'].includes(ext)) return 'text';
  return 'file';
}

/** `@` 选文件交过来的收不收（蓝图 `web.md`「`@` 选文件」第 5 条）：图片、PDF、音频、视频收成附件，别的照路径写进话里。 */
export function attachable(file) {
  return ['image', 'pdf', 'audio', 'video'].includes(kindOf(file));
}

/** 文字文件有几行：最后一行没有换行也算一行；空的是 0 行（卡片上那一行小字，蓝图「附件」第 3 条）。 */
export function lineCount(text) {
  if (!text) return 0;
  const n = text.split('\n').length;
  return text.endsWith('\n') ? n - 1 : n;
}

/** 卡左下角的标签：扩展名大写；没有扩展名的（`Makefile`、`.bashrc`）是空的（第 3 条）。 */
export function extOf(name) {
  const dot = name.lastIndexOf('.');
  return dot > 0 ? name.slice(dot + 1).toUpperCase() : '';
}

/** 框里那一排：照先后的几块，变了告诉看着的。 */
export class Tray {
  constructor() {
    /** @type {Item[]} */
    this.items = [];
    this.seq = 0;
    /** @type {Set<() => void>} */
    this.watchers = new Set();
    /** 交出去的几块，拒了照它放回来；不是这里交出去的不认 */
    /** @type {WeakMap<Taken, Item[]>} */
    this.given = new WeakMap();
  }

  /** 看着变。交回怎么不看。 */
  watch(fn) {
    this.watchers.add(fn);
    return () => this.watchers.delete(fn);
  }

  changed() {
    for (const fn of this.watchers) fn();
  }

  /**
   * 放进来一个，先是在传。
   * @param {FileLike} file
   */
  add(file) {
    /** @type {Item} */
    const item = { id: ++this.seq, file, name: file.name, size: file.size, state: 'uploading', put: null };
    this.items.push(item);
    this.changed();
    return item;
  }

  /**
   * 传完了。已经拿掉的交回 `false`（不收）。
   * @param {number} id
   * @param {Put} put
   */
  ready(id, put) {
    const item = this.items.find((it) => it.id === id);
    if (!item) return false;
    item.state = 'ready';
    item.put = put;
    this.changed();
    return true;
  }

  /** 拿掉一块（传到一半的也能拿）；交回拿掉的那块。 */
  remove(id) {
    const at = this.items.findIndex((it) => it.id === id);
    if (at < 0) return null;
    const [item] = this.items.splice(at, 1);
    this.changed();
    return item;
  }

  /** 键盘拿掉最后一块（光标在最前面按退格，蓝图「附件」第 3 条）：拿掉了交回 `true`，一块都没有的 `false`。 */
  dropLast() {
    const last = this.items.at(-1);
    return last ? !!this.remove(last.id) : false;
  }

  has() {
    return this.items.length > 0;
  }

  /** 还有在传的：这时不能发。 */
  busy() {
    return this.items.some((it) => it.state === 'uploading');
  }

  /**
   * 发：交出全部（只要 blob、名字、媒体类型）、框里清空。没有的、还在传的交 `null`。
   * @returns {Taken|null}
   */
  take() {
    if (!this.has() || this.busy()) return null;
    const items = this.items;
    const taken = { attachments: items.map((it) => {
      const put = /** @type {Put} */ (it.put);
      return { blob: put.blob, name: put.name, media_type: put.media_type };
    }) };
    this.given.set(taken, items);
    this.items = [];
    this.changed();
    return taken;
  }

  /**
   * 交出去的记进输入历史的样子（蓝图 `web.md`「输入历史」第 1 条）：只留核心存好的那一份、名字、媒体类型、大小，浏览器里的
   * 文件存不下。不是这里交出去的交 `null`。
   * @param {Taken} taken
   * @returns {Kept[]|null}
   */
  keep(taken) {
    const items = this.given.get(taken);
    if (!items) return null;
    return items.map((it) => {
      const put = /** @type {Put} */ (it.put);
      return { blob: put.blob, name: put.name, media_type: put.media_type, size: it.size };
    });
  }

  /**
   * 输入历史翻出来的（第 2 条）：换掉上一次跟着翻出来、还没留下的几块，自己放进来的不动；`null` 是只拿掉。翻出来的是传好了的
   * （核心存好的那一份，再发不用重新传），文件带着它（缩略图照它取）。
   * @param {Kept[]|null} kept
   * @param {string|null} session 发在哪个会话（取那一份照它）
   */
  recall(kept, session) {
    this.items = this.items.filter((it) => !it.recalled);
    for (const k of kept ?? []) {
      const file = { name: k.name, size: k.size, type: k.media_type, stored: { session: /** @type {string} */ (session), hash: k.blob } };
      this.items.push({ id: ++this.seq, file, name: k.name, size: k.size, state: 'ready', put: { blob: k.blob, name: k.name, media_type: k.media_type }, recalled: true });
    }
    this.changed();
  }

  /** 翻出来的字改了、从列表里选定了：跟着回来的几块留下，当成自己放的。 */
  settle() {
    for (const it of this.items) delete it.recalled;
  }

  /**
   * 核心拒了：交出去的放回来，排在现在框里的前面（原来就在前面）。
   * @param {Taken} taken
   */
  putBack(taken) {
    const items = this.given.get(taken);
    if (!items) return;
    this.given.delete(taken);
    this.items = [...items, ...this.items];
    this.changed();
  }
}
