// @ts-check
//! 「软件包」页上面「核心」那一段（蓝图 `web.md`「设置页」第 13 条）：核心认的软件包一行一个，和下面「网页」那一段一个样子
//! （2026-10-09 项目主人：原来核心的包全摊开、只有 QQ 那个是展开的，看着不一致）：箭头、名字、一句说明（`package.list` 的 `summary`）；
//! 扩展在名字后面暗色写现在怎样（关着、在运行……）。点开：扩展先是「运行」「权限」两行（`extensions.js`），再是这个包的设置项（照
//! `config.schema` 那一组）。没设置项、又不是扩展的不列；设置项画在别的页的包这里也不列。通讯平台的接入（接入QQ）也在这里：核心的
//! 「接入」页照设置项 `merge` 并进这一页（2026-10-10 项目主人：主菜单去掉「接入」，以后软件包页管软件的信息和开关、「软件后台」页
//! 跳到软件自己的页面，等核心出形状）。

import { h, icon } from '../../src/lib/dom.js';
import { coreRow, groupBlock } from './rows.js';
import { stateText } from './extensions.js';

/** 点开着的包：换页回来还开着（这个弹窗里记着） */
const opened = new Set();

/** @param {any} dialog @param {any} page `config.schema` 的 `packages` 页 */
export function drawCorePackages(dialog, page) {
  const ctx = dialog.ctx;
  const groups = page?.groups ?? [];
  const ext = dialog.extensions;
  // 设置项画在别的页的包不在这里列（照并过以后的页算：并进这一页的「接入」算这里）
  const elsewhere = new Set((dialog.pages ?? []).filter((p) => p.id !== 'packages').flatMap((p) => p.groups.map((g) => g.id)));
  const packages = (dialog.packages ?? []).filter((p) => !elsewhere.has(p.package));
  const rows = packages.map((p) => {
    const id = p.package;
    const group = groups.find((g) => g.id === id);
    const isExt = ext.entries.has(id);
    if (!group && !isExt) return null;
    const settings = (group?.items ?? []).map((item) => coreRow(dialog, item)).filter(Boolean);
    const open = opened.has(id);
    const entry = ext.entries.get(id);
    const body = h('div.set-pkg-settings', { hidden: !open }, open ? (isExt ? ext.body(id, settings) : h('div.set-rows', settings)) : null);
    const head = h(`div.set-row.set-pkg.is-expandable${open ? '.is-open' : ''}`, {
      onclick: () => {
        const next = !opened.has(id);
        if (next) opened.add(id);
        else opened.delete(id);
        head.classList.toggle('is-open', next);
        body.hidden = !next;
        if (next && !body.childElementCount) body.append(isExt ? ext.body(id, settings) : h('div.set-rows', settings));
      },
    },
    h('span.set-pkg-arrow', icon('chevron-right')),
    h('div.set-text',
      h('div.set-name', h('span', p.name ?? id), entry ? h('em.set-pkg-state', stateText((k, f) => ctx.text(k, f), entry)) : null),
      p.summary ? h('p.set-desc', p.summary) : null));
    return h('div.set-pkg-block', head, body);
  }).filter(Boolean);
  // 对不上哪个包的组（核心自己的模块）照旧一组一组摊开
  const all = new Set((dialog.packages ?? []).map((p) => p.package));
  const rest = groups.filter((g) => !all.has(g.id)).map((g) => groupBlock(g.name, g.items.map((item) => coreRow(dialog, item)))).filter(Boolean);
  return [rows.length ? h('section.set-group', h('div.set-rows', rows)) : null, ...rest];
}
