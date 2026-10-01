// @ts-check
//! 换模型的菜单要列什么（蓝图 `web.md`「换模型的菜单」，照 Claude 网页端的模型菜单）：照核心的 `model.list`（核心施工 8-7、
//! 8-8、8-9）排出模型、模型池两页；框下面那一截怎么拆。纯函数，画在 `ui/model-menu.js`。

import { t } from '../util/res.js';
import { hhmm } from './format.js';

/**
 * @typedef {{ref: string, title: string, desc: string, current: boolean, usable: boolean, why: string}} Row 一行：选了交给
 *   核心的引用、上面一行、下面一行小字、是不是现在用着的、能不能选、不能选为什么（悬停写）
 */

/**
 * @param {any} list `model.list` 的回应；还没有的是 `null`
 * @param {string|null} current 会话现在的引用（选过还没生效的照选的）
 * @returns {{models: Row[], pools: Row[]}}
 */
export function menuOf(list, current) {
  if (!list) return { models: [], pools: [] };
  const models = (list.providers ?? []).flatMap((p) => (p.models ?? []).map((m) => {
    const usable = m.state === 'ok';
    return { ref: m.ref, title: m.model, desc: p.id, current: m.ref === current, usable, why: usable ? '' : why(m) };
  }));
  const pools = (list.pools ?? []).map((pool) => {
    const ref = `@${pool.name}`;
    const members = pool.models.map((r) => footerOf(r).model).join(t('list_sep'));
    const how = t(`model_menu.${pool.strategy === 'rotate' ? 'rotate' : 'pin'}`);
    return { ref, title: ref, desc: `${how} · ${members}`, current: ref === current, usable: true, why: '' };
  });
  return { models, pools };
}

/** 用不了的为什么：冷却到几点（本地时间）、没设 key。 */
function why(m) {
  if (m.state === 'cooling') return t('model_menu.cooling', { time: m.until ? hhmm(m.until) : '' });
  return t('model_menu.no_key');
}

/**
 * 框下面那一截：引用照第一个 `/` 拆成模型名和供应商（模型名里可以带 `/`）；池、挡位名照原样写，不写供应商。
 * @param {string} ref
 * @returns {{model: string, endpoint: string|null}}
 */
export function footerOf(ref) {
  const cut = ref.startsWith('@') ? -1 : ref.indexOf('/');
  return cut > 0 ? { model: ref.slice(cut + 1), endpoint: ref.slice(0, cut) } : { model: ref, endpoint: null };
}
