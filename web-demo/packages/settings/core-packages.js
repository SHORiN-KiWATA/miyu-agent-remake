// @ts-check
//! 「软件包」页上面核心的软件包（蓝图 `web.md`「设置页」第 13 条；核心 F-6 上，`package-pages.md`）：照 `package.list` 装了的一个一行，
//! 按核心核实过的标记分段：接入（带 `connection` 的）、界面（`ui`）、功能（别的）。一行是图标、名字、一句说明，右边状态、开关、箭头：
//! 状态只写要人留意的（运行中、已停止、程序未安装……；好好的「已启用」、关着的「已停用」开关已经说了，不写），开关照 `enabled`
//! （有这一格才有开关，发 `package.enable`、`package.disable`，核心照种类办），必需的写「必需」。点一行滑进它的信息页（`subpage.js`，
//! 左上角箭头返回；2026-10-10 项目主人）：版本和状态、启用、它加的功能、权限（扩展的「运行」「权限」两行，`extensions.js`）、
//! 设置项（`config.schema` 里 `package` 是它的）、卸载（管理员自己装的才有）。程序不在的只有状态（界面包核心照旧给设置项，照画）。

import { h, icon, hasIcon } from '../../src/lib/dom.js';
import { coreRow, groupBlock, toggle } from './rows.js';

/** 段：照核心核实过的标记分，先后照这里 */
const PARTS = /** @type {const} */ (['connection', 'ui', 'feature']);
/** 列表上不写的状态：开关已经说了 */
const QUIET = new Set(['ready', 'off']);

/** @param {any} p */
const partOf = (p) => (p.connection ? 'connection' : p.kind === 'ui' ? 'ui' : 'feature');

/** 包的图标：清单写了、网页认得的照它，不然画一个通用的。 @param {any} p */
const faceOf = (p) => h('span.set-pkg-face', icon(p.icon && hasIcon(p.icon) ? p.icon : 'package'));

/** @param {any} dialog @param {any} page `config.schema` 的 `packages` 页 */
export function drawCorePackages(dialog, page) {
  const t = (/** @type {string} */ key, /** @type {any} */ fields) => dialog.ctx.text(key, fields);
  const packages = (dialog.packages ?? []).filter((p) => p.status);
  const parts = PARTS.map((part) => {
    const rows = packages.filter((p) => partOf(p) === part).map((p) => packageRow(dialog, p));
    return rows.length ? h('section.set-part', h('h2.set-part-name', t(`pkg.part.${part}`)), h('section.set-group', h('div.set-rows', rows))) : null;
  });
  // 对不上哪个包的组（核心自己的模块）照旧一组一组摊开
  const all = new Set(packages.map((p) => p.package));
  const rest = (page?.groups ?? []).filter((g) => !all.has(g.id)).map((g) => groupBlock(g.name, g.items.map((item) => coreRow(dialog, item)))).filter(Boolean);
  return [...parts, ...rest].filter(Boolean);
}

/**
 * 扩展的状态变了（订阅 `extensions` 推来的 `extension.changed`）：列表里那一项的状态、开关跟着换（核心不另推软件包的变化）。
 * 状态照图纸：关着的 `off`，`waiting` 算 `starting`。交回换没换。
 * @param {any} dialog @param {any} entry `extension.status` 的一项
 */
export function followExtension(dialog, entry) {
  const p = (dialog.packages ?? []).find((x) => x.package === entry?.package);
  if (!p || p.status === 'program_missing') return false;
  const status = !entry.on ? 'off' : entry.state === 'waiting' ? 'starting' : entry.state;
  if (p.status === status && p.enabled === entry.on) return false;
  dialog.packages = dialog.packages.map((x) => (x === p ? { ...x, status, enabled: entry.on } : x));
  return true;
}

/** 列表上的一行。 @param {any} dialog @param {any} p */
function packageRow(dialog, p) {
  const t = (/** @type {string} */ key) => dialog.ctx.text(key);
  const id = p.package;
  const missing = p.status === 'program_missing';
  const state = QUIET.has(p.status) ? null : h(`span.set-pkg-status${missing ? '.is-bad' : p.status === 'running' ? '.is-on' : ''}`, t(`pkg.status.${p.status}`));
  const control = p.required ? h('span.set-pkg-status', t('pkg.required'))
    : 'enabled' in p ? switchFor(dialog, p) : null;
  return h('div.set-row.set-pkg.is-expandable', {
    onclick: (/** @type {MouseEvent} */ e) => {
      if (/** @type {Element} */ (e.target).closest('.set-switch')) return;
      openInfo(dialog, id);
    },
  },
  faceOf(p),
  h('div.set-text', h('div.set-name', h('span', p.name ?? id)), p.summary ? h('p.set-desc', p.summary) : null),
  state, control ? h('div.set-control', control) : null,
  h('span.set-pkg-arrow', icon('chevron-right')));
}

/** 进一个包的信息页。 @param {any} dialog @param {string} id */
function openInfo(dialog, id) {
  const p = (dialog.packages ?? []).find((x) => x.package === id);
  dialog.openSub(p?.name ?? id, () => infoPage(dialog, id));
}

/**
 * 开关：`package.enable`、`package.disable`，回应是那一项，换进列表重画。扩展还有没批的能力（`needs_approval`）的进它的信息页、
 * 展开要批准的那一块（批了照 `extensions.js` 开）。程序不在的灰着。
 * @param {any} dialog @param {any} p
 */
function switchFor(dialog, p) {
  const id = p.package;
  const el = toggle(!!p.enabled, async (on) => {
    try {
      const got = await dialog.ctx.core.request(on ? 'package.enable' : 'package.disable', { package: id });
      dialog.packages = dialog.packages.map((x) => (x.package === id ? got : x));
      dialog.drawBody();
      return true;
    } catch (err) {
      if (err?.reason === 'needs_approval') {
        dialog.extensions.asking.add(id);
        if (!dialog.sub.page) openInfo(dialog, id);
        else dialog.drawBody();
      } else dialog.toast(err?.message ?? String(err));
      return false;
    }
  });
  if (p.status === 'program_missing') el.setAttribute('disabled', '');
  return el;
}

/**
 * 一个包的信息页（点进去的那一层）：每次画都照现在的列表、配置清单重取，开关、别处改了配置重画时跟着变。
 * @param {any} dialog @param {string} id
 */
function infoPage(dialog, id) {
  const t = (/** @type {string} */ key, /** @type {any} */ fields) => dialog.ctx.text(key, fields);
  const p = (dialog.packages ?? []).find((x) => x.package === id);
  if (!p) return [h('p.set-empty', t('pkg.gone'))];
  const missing = p.status === 'program_missing';
  // 状态和版本：好好的「已启用」、关着的「已停用」开关已经说了，不写
  const line = [QUIET.has(p.status) ? null : t(`pkg.status.${p.status}`), p.version ? t('pkg.version', { version: p.version }) : null].filter(Boolean).join(' · ');
  const head = [
    p.summary ? h('p.set-sub-desc', p.summary) : null,
    line ? h(`p.set-pkg-line${missing ? '.is-bad' : ''}`, line) : null,
  ];
  const group = (dialog.pages ?? []).find((x) => x.id === 'packages')?.groups.find((g) => g.id === id);
  const settings = (group?.items ?? []).map((item) => coreRow(dialog, item)).filter(Boolean);
  // 程序不在的：没有开关、功能；设置项照核心给不给（扩展、小程序不给，界面照旧给）
  if (missing) return [...head, settings.length ? groupBlock(t('pkg.settings'), settings) : null, removeButton(dialog, p)];
  const isExt = dialog.extensions.entries.has(id);
  // 只有一个功能、名字和软件一样的（人格记忆这种）不列：和页头说的重了
  const own = (p.features ?? []).length === 1 && p.features[0].name === p.name;
  const features = (own ? [] : p.features ?? []).map((f) => h('div.set-row', h('div.set-text', h('div.set-name', h('span', f.name ?? f.id)), f.summary ? h('p.set-desc', f.summary) : null)));
  return [
    ...head,
    // 扩展：运行、权限、要批准的那一块，再接设置项；别的：启用一行（有开关的），设置项另起一组
    isExt ? h('section.set-group', dialog.extensions.body(id, settings))
      : 'enabled' in p ? h('section.set-group', h('div.set-rows', h('div.set-row', h('div.set-text', h('div.set-name', h('span', t('enabled')))), h('div.set-control', switchFor(dialog, p))))) : null,
    features.length ? groupBlock(t('pkg.features'), features) : null,
    !isExt && settings.length ? groupBlock(t('pkg.settings'), settings) : null,
    removeButton(dialog, p),
  ];
}

/**
 * 卸载：只有管理员自己装的、不是必需的才有（出厂的卸掉和关掉是一回事，照列表的开关）。点一下变成「再点一次卸载」，再点才卸。
 * @param {any} dialog @param {any} p
 */
function removeButton(dialog, p) {
  if (p.required || p.layer === 'shipped') return null;
  const t = (/** @type {string} */ key) => dialog.ctx.text(key);
  let armed = false;
  const button = h('button.set-btn.is-danger', { type: 'button', onclick: async () => {
    if (!armed) {
      armed = true;
      button.textContent = t('pkg.uninstall_again');
      return;
    }
    try {
      await dialog.ctx.core.request('package.remove', { package: p.package });
      dialog.back();
      await dialog.reload();
    } catch (err) {
      dialog.toast(err?.message ?? String(err));
    }
  } }, t('pkg.uninstall'));
  return h('div.set-pkg-foot', button);
}
