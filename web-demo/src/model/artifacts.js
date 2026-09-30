// @ts-check
//! 预览工作区里放什么（蓝图 `web.md`「预览工作区」第 1 条）：这个会话里 `write`、`edit` 改过的文件，路径落在工作目录下约定的
//! 目录（`artifacts.json` 的 `dir`）里的。
//!
//! 设计上产物是 `artifact`、`present_artifact` 两件工具，网页头经提供者接口提供（设计 10 第四节）；接口排在 M9 之后，演示先让
//! 她用 `write` 写进约定的目录（2026-09-30 项目主人定）。照日志认，不监视目录：日志是真相，刷新以后照样重建（和核心施工的会话定）。
//!
//! - 效果里的路径是换成真实位置以后的绝对路径（`kernel/events-bodies.md`「效果」），约定的目录也要先换成真实位置再比（桥的
//!   `web.realpath`）；
//! - 撤销掉的那几轮的效果不算，恢复了又算；删掉的（`file.trashed`）不算，删了又写的算；
//! - 一个文件一项，最后改的在前。

import { res } from '../util/res.js';

/**
 * @typedef {{path: string, name: string, kind: string, seq: number, turn: number|null}} Artifact
 *   `name` 是相对约定目录的路径；`kind` 是 `markdown`、`html`、`image`、`video`、`code`、`text`
 */

/**
 * 这个会话的产物。`dir` 是约定目录的真实位置；还不知道的（桥还没回）是 `null`，一项都没有。
 * @param {any[]} events 持久事件，照序号
 * @param {string|null} dir
 * @returns {Artifact[]}
 */
export function artifacts(events, dir) {
  if (!dir) return [];
  const prefix = dir.endsWith('/') ? dir : `${dir}/`;
  const reverted = new Set();
  /** @type {{path: string, seq: number, turn: number|null, gone: boolean}[]} */
  const changes = [];
  for (const e of events) {
    if (e.kind === 'turn.reverted') for (const n of e.body.turns) reverted.add(n);
    if (e.kind === 'turn.unreverted') for (const n of e.body.turns) reverted.delete(n);
    if (e.kind !== 'tool.result') continue;
    for (const fx of e.body.effects ?? []) {
      if ((fx.kind === 'file.changed' || fx.kind === 'file.trashed') && fx.path?.startsWith(prefix)) {
        changes.push({ path: fx.path, seq: e.seq, turn: e.turn ?? null, gone: fx.kind === 'file.trashed' });
      }
    }
  }
  /** @type {Map<string, {seq: number, turn: number|null, gone: boolean}>} 每个文件最后那一次（撤销掉的不算） */
  const last = new Map();
  for (const c of changes) if (!reverted.has(c.turn)) last.set(c.path, c);
  return [...last.entries()]
    .filter(([, c]) => !c.gone)
    .sort((a, b) => b[1].seq - a[1].seq)
    .map(([path, c]) => ({ path, name: path.slice(prefix.length), kind: artifactKind(path), seq: c.seq, turn: c.turn }));
}

/** 照扩展名认类型（`artifacts.json` 的 `kinds`）；认不出的当文字。 */
export function artifactKind(path) {
  const ext = /\.([^./]+)$/.exec(path)?.[1]?.toLowerCase() ?? '';
  for (const [kind, exts] of Object.entries(res.artifacts.kinds)) if (exts.includes(ext)) return kind;
  return 'text';
}

/** 代码按哪种语言上色（`artifacts.json` 的 `languages`）；没有的是 `null`，不上色。 */
export function artifactLanguage(path) {
  const ext = /\.([^./]+)$/.exec(path)?.[1]?.toLowerCase() ?? '';
  return res.artifacts.languages[ext] ?? null;
}
