// @ts-check
//! `@` 选文件问核心（蓝图 `web.md`「`@` 选文件」第 2 条，核心施工 W-2；原来由桥的 `web.files` 列、找）：按目录找经 `fs.list`，
//! 模糊找经 `fs.find`，都带这个会话的工作目录。清单没建完（`building`）的先交已经建好的，隔 `find_poll_ms` 再问，直到建完。
//! 核心的列表里不带媒体类型：照扩展名补上（`resources/media.json` 的 `types`），附件包照它认收不收。

import { res } from '../util/res.js';

/**
 * @typedef {{path: string, full: string, dir: boolean, marks: number[], size?: number, type: string}} Entry
 * @typedef {{items: Entry[], partial: boolean, layer: boolean}} Found `layer` 是按目录找的（只读这一层）
 */

/** 照扩展名认媒体类型；目录、认不得的是空的。 @param {{path: string, dir: boolean}} x */
function typeOf(x) {
  if (x.dir) return '';
  const dot = x.path.lastIndexOf('.');
  return dot < 0 ? '' : res.media?.types?.[x.path.slice(dot + 1).toLowerCase()] ?? '';
}

/** @param {any} got @param {boolean} layer @returns {Found} */
function shape(got, layer) {
  return { items: (got?.items ?? []).map((x) => ({ ...x, type: typeOf(x) })), partial: !!got?.partial, layer };
}

/**
 * 列、找一次。模糊找的清单没建完：每一次的结果先交给 `onUpdate`，隔一会儿接着问（不再带 `fresh`），`stale()` 说不要了就停。
 * 问不到的照原样报出去（`model/mention.js` 的 `failure` 写成人话）。
 * @param {import('./connection.js').Connection} conn
 * @param {string|null} cwd 这个会话的工作目录
 * @param {{mode: 'dir', dir: string, prefix: string}|{mode: 'find', query: string}} plan `model/mention.js` 的 `plan`
 * @param {boolean} fresh 开列表时：清单旧了重建
 * @param {(found: Found) => void} [onUpdate]
 * @param {() => boolean} [stale]
 * @returns {Promise<Found>}
 */
export async function listFiles(conn, cwd, plan, fresh, onUpdate = () => {}, stale = () => false) {
  if (plan.mode === 'dir') return shape(await conn.request('fs.list', { cwd, dir: plan.dir, prefix: plan.prefix }), true);
  let ask = fresh;
  for (;;) {
    const got = await conn.request('fs.find', { cwd, query: plan.query, fresh: ask });
    const found = shape(got, false);
    if (!got?.building || stale()) return found;
    onUpdate(found);
    ask = false;
    await new Promise((r) => setTimeout(r, res.layout.find_poll_ms));
  }
}
