// @ts-check
//! 她的回答里的图、卡片接进 Markdown 的扩展点（`markdown/render.js` 的 `Hooks`）：蓝图 `web.md`「mermaid 图」「图片」
//! 「音视频、图片卡片」「链接卡片」。
//!
//! 本机的地址（绝对路径、`~/`、`file://`、相对工作目录的）经桥的 `/file` 取，要这个会话的编号；还没开的新会话取不了，照原文写。
//!
//! 在收的回答每来一段字整段重画：图、卡片、播放器每次重造会一闪一闪（图重载、视频从头来）。画过的节点按「它是什么」
//! 记着，下一次重画时同一个的直接挪过去用（一次重画里同一个出现几次的，照第几次分开记）。记的范围是一条回答（`scope`：
//! 回合加这一轮里第几段回答，在收的落了盘也还是它）：两条回答里有同一张图，各用各的。
//! 网上的地址照原样给浏览器（图片也是，蓝图「图片」第 1 条待拍板）。

import { imageCard, videoCard, audioCard } from './media.js';
import { scanLinks } from './linkcards.js';
import { mediaLine } from '../model/cards.js';
import { localPath } from '../model/paths.js';
import { fileUrl } from '../core/host.js';

/**
 * 看着的这个会话在哪：会话编号（新会话还没开的是 `null`）、家目录、工作目录。
 * @typedef {{session: string|null, home: string|null, cwd: string|null}} Where
 */

/**
 * 这个会话用的扩展点：交回一个函数，给一条回答的范围（`scope`）交回这一条用的扩展点。
 * @param {Where} where
 * @param {(text: string, good?: boolean) => void} say 提示（复制了几个字）
 * @param {Ext} [ext] 软件包接进来的：挂载位（代码块照语言分派给挂进 `markdown.code` 的包，比如 mermaid）、现在的灯箱
 * @returns {(scope: string) => import('../markdown/render.js').Hooks}
 */
export function richHooks(where, say, ext) {
  /** 画过的节点：`范围|种类:内容#第几次` → 节点。 */
  const made = new Map();
  return (scope) => hooksFor(scope, where, say, made, ext);
}

/**
 * @typedef {{slots?: any, lightbox?: () => any, storage?: {get: (k: string, d: any) => any, set: (k: string, v: any) => void}, account?: string|null, titleOf?: (id: string) => string|null}} Ext
 *   软件包 app 交进来的：`ctx.slots`、交回现在的灯箱（没装是 `undefined`）、这台设备上存东西（内核的 `storage`）、这个页面登录成的账号
 */

/** 一条回答用的扩展点（见 [`richHooks`]）。 */
function hooksFor(scope, where, say, made, ext) {
  const slots = ext?.slots;
  /** 这一次重画里每一种出现了几次（`after` 时清零：一次重画完了）。 */
  let counts = new Map();
  /** 同一个的拿记着的，没有的造一个记下来。 */
  const reuse = (key, build) => {
    const n = (counts.get(key) ?? 0) + 1;
    counts.set(key, n);
    const id = `${scope}|${key}#${n}`;
    if (!made.has(id)) {
      const node = build();
      if (!node) return null;
      made.set(id, node);
    }
    return made.get(id);
  };
  /** 地址 → 浏览器能取的：本机的换成桥的地址；网上的照原样；别的（取不了的）是 `null`。 */
  const url = (target, download = false) => {
    const path = localPath(target, where);
    if (path) return where.session ? fileUrl(where.session, path, download) : null;
    return /^https?:\/\//i.test(target) ? target : null;
  };
  return {
    // 收齐了的才交给挂进 `markdown.code` 的包（照语言）；没收齐的、没人接的照代码块写。记的时候带上是哪一次挂的，包重装了重画
    code: (lang, text, closed) => {
      const entry = closed ? slots?.pick('markdown.code', lang.toLowerCase()) : null;
      if (!entry || entry.key === '*') return null;
      return reuse(`code:${entry.seq}:${text}`, () => {
        const out = slots.draw(entry, { text, say });
        // 画法抛错的（`draw` 兜着交回 `{failed}`）照代码块写
        return out && typeof out === 'object' && 'nodeType' in out ? out : null;
      });
    },
    line: (line) => {
      const m = mediaLine(line);
      const src = m && url(m.target);
      if (!m || !src) return null;
      return reuse(`line:${line.trim()}`, () => {
        if (m.kind === 'video') return videoCard({ url: src, name: m.label });
        if (m.kind === 'audio') return audioCard({ url: src, name: m.label, download: url(m.target, true) ?? undefined });
        return imageCard({ url: src, name: m.label, lightbox: ext?.lightbox });
      });
    },
    image: (src, alt) => {
      const got = url(src);
      return got ? reuse(`image:${src}\n${alt}`, () => imageCard({ url: got, name: alt || src.split('/').pop() || src, lightbox: ext?.lightbox })) : null;
    },
    after: (container) => {
      counts = new Map();
      scanLinks(container);
    },
  };
}
