// @ts-check
//! 「软件包」页上面「核心」那一段（蓝图 `web.md`「设置页」第 13 条）：核心认的软件包一行一个，和下面「网页」那一段一个样子
//! （2026-10-09 项目主人：原来核心的包全摊开、只有 QQ 那个是展开的，看着不一致）：名字、一句说明（`package.list` 的 `summary`）、右边一个箭头；
//! 扩展在名字后面暗色写现在怎样（关着、在运行……）。点一行滑进这个包的设置页（`subpage.js`，左上角箭头返回）：扩展先是「运行」
//! 「权限」两行（`extensions.js`），再是这个包的设置项（照 `config.schema` 那一组）。没设置项、又不是扩展的不列；设置项画在别的页的包这里也不列。通讯平台的接入（接入QQ）也在这里：核心的
//! 「接入」页照设置项 `merge` 并进这一页（2026-10-10 项目主人：主菜单去掉「接入」，以后软件包页管软件的信息和开关、「软件后台」页
//! 跳到软件自己的页面，等核心出形状）。

import { h, icon } from '../../src/lib/dom.js';
import { coreRow, groupBlock } from './rows.js';
import { stateText } from './extensions.js';

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
    if (!groups.some((g) => g.id === id) && !ext.entries.has(id)) return null;
    const entry = ext.entries.get(id);
    // 点这一行滑进它的设置页（2026-10-10 项目主人：滑动进入软件包的设置页，左上角箭头返回）
    return h('div.set-row.set-pkg.is-expandable', { onclick: () => dialog.openSub(p.name ?? id, () => corePackagePage(dialog, p)) },
      h('div.set-text',
        h('div.set-name', h('span', p.name ?? id), entry ? h('em.set-pkg-state', stateText((k, f) => ctx.text(k, f), entry)) : null),
        p.summary ? h('p.set-desc', p.summary) : null),
      h('span.set-pkg-arrow', icon('chevron-right')));
  }).filter(Boolean);
  // 对不上哪个包的组（核心自己的模块）照旧一组一组摊开
  const all = new Set((dialog.packages ?? []).map((p) => p.package));
  const rest = groups.filter((g) => !all.has(g.id)).map((g) => groupBlock(g.name, g.items.map((item) => coreRow(dialog, item)))).filter(Boolean);
  return [rows.length ? h('section.set-group', h('div.set-rows', rows)) : null, ...rest];
}

/**
 * 一个核心软件包的设置页（点进去的那一层）：顶上一句说明，扩展先是「运行」「权限」两行（`extensions.js`），再是它的设置项。
 * 每次画都照现在读到的配置清单重取，别处改了配置重画时跟着变。
 * @param {any} dialog @param {any} p `package.list` 的一项
 */
function corePackagePage(dialog, p) {
  const id = p.package;
  const group = (dialog.pages ?? []).find((x) => x.id === 'packages')?.groups.find((g) => g.id === id);
  const settings = (group?.items ?? []).map((item) => coreRow(dialog, item)).filter(Boolean);
  return [
    p.summary ? h('p.set-sub-desc', p.summary) : null,
    h('section.set-group', dialog.extensions.entries.has(id) ? dialog.extensions.body(id, settings) : h('div.set-rows', settings)),
  ];
}
