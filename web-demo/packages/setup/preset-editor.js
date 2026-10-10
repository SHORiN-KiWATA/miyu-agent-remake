// @ts-check
//! 预设的详情（蓝图 `web.md`「人格、预设、工作区」第 6 条）：浮起来的卡片里（`float.js`）——名字、功能开关（功能照核心 F-3 下的 `features`：
//! 一个功能一个开关，有几件工具的把工具平铺在下面、一件一个开关——2026-10-09 项目主人：「每一个工具都是一个功能，直接平铺，用组名做分隔线」，
//! 和终端对齐；2026-10-08 项目主人：预设不要说明，
//! 「没什么意义」；不带默认人格，人格、预设互不引用）。都是点了当场存（`preset.set` 只带改的那一项，不带 `expect`）：名字回车、离开时存，开关点了就存；功能、工具照核心给的
//! 名字（照连接的语言），没装的写「没安装」。删除照核心的 `remove`，同人格。

import { h, replace } from '../../src/lib/dom.js';
import { field, twoClick } from './form.js';
import { refusalText } from './persona-editor.js';

/** @typedef {{saved: () => void, removed: (remains: boolean) => void, close: () => void}} Hooks */

export class PresetEditor {
  /**
   * @param {any} ctx @param {import('./form.js').Kit} kit @param {import('./catalog.js').Catalog} catalog @param {string} id @param {Hooks} hooks
   * @param {{name?: string}} [look] 列表里那一块写的（读完以前卡片头上先写它的名字）
   */
  constructor(ctx, kit, catalog, id, hooks, look = {}) {
    this.ctx = ctx;
    this.kit = kit;
    this.catalog = catalog;
    this.id = id;
    this.hooks = hooks;
    this.t = (/** @type {string} */ key, /** @type {any} */ fields) => ctx.text(key, fields);
    this.title = h('span', look.name ?? id);
    this.body = h('div.setup-editor', h('p.setup-empty', this.t('page.loading')));
  }

  async load() {
    try {
      this.draw(await this.ctx.core.request('preset.get', { preset: this.id }));
    } catch (err) {
      replace(this.body, h('p.setup-error', refusalText(err)));
    }
  }

  /** 都是当场存的，没有改了没存的。 */
  dirty() {
    return false;
  }

  /** 点外面、`Esc`、✕：都是当场存的，直接关。 */
  tryClose() {
    this.hooks.close();
  }

  /** @param {any} got `preset.get` 的回应 */
  draw(got) {
    const t = this.t;
    const label = typeof got.name === 'string' ? got.name : '';
    this.title.textContent = label || this.id;
    const name = this.kit.text(label, '', (text) => { if (text) this.set([{ key: 'preset.name', value: text }]); });
    const list = Array.isArray(got.features) ? got.features : [];
    const features = h('div.setup-features', list.map((f) => this.featureRow(f)));
    this.note = h('p.setup-error', { hidden: true });
    const remove = got.remove ? twoClick(this.kit, t(`edit.${got.remove}`), t(`edit.${got.remove}_again`), () => this.drop()) : null;
    replace(this.body,
      field(t('edit.name'), name),
      field(t('edit.features'), list.length ? features : h('p.setup-empty', t('edit.no_features'))),
      this.note,
      remove ? h('div.setup-foot', remove) : null);
  }

  /**
   * 一个功能一行：名字（悬停看说明）、开关；没装的写「没安装」。有不止一件工具的，工具平铺在下面、缩进一格，一件一个开关（设计 30 第四节：
   * `[tools]` 能在开着的功能里关掉单件）；功能关着时工具暗着、点不了。开关写 `features.<编号>`，单件写 `tools.<工具名>`，编号、工具名不往界面上露。
   * @param {any} f
   */
  featureRow(f) {
    if (f.installed === false) return h('div.setup-feature.is-missing', h('span.setup-feature-name', f.name ?? f.id), h('span.setup-missing', this.t('edit.not_installed')));
    const row = h('div.setup-feature', h('span.setup-feature-name', { title: f.summary ?? null }, f.name ?? f.id),
      this.kit.toggle(!!f.on, (on) => this.set([{ key: `features.${f.id}`, value: on }])));
    const tools = f.tools ?? [];
    if (tools.length < 2) return row;
    return [row, h(`div.setup-tools${f.on ? '' : '.is-off'}`, tools.map((tool) => {
      const toggle = this.kit.toggle(!!tool.on, (on) => this.set([{ key: `tools.${tool.name}`, value: on }]));
      if (!f.on) toggle.setAttribute('disabled', '');
      return h('div.setup-tool', h('span', tool.label ?? tool.name), toggle);
    }))];
  }

  /** 改一项：成了照回应重画（回应同 `preset.get`），拒了写原因；交回存没存成（开关照它拨回去）。 @param {any[]} changes */
  async set(changes) {
    try {
      const got = await this.ctx.core.request('preset.set', { preset: this.id, changes });
      this.draw(got);
      this.hooks.saved();
      return true;
    } catch (err) {
      if (this.note) {
        this.note.hidden = false;
        this.note.textContent = refusalText(err);
      }
      return false;
    }
  }

  async drop() {
    try {
      const got = await this.ctx.core.request('preset.delete', { preset: this.id });
      this.kit.toast(this.t(got?.remains ? 'edit.restored' : 'edit.deleted'));
      this.hooks.removed(!!got?.remains);
    } catch (err) {
      if (this.note) {
        this.note.hidden = false;
        this.note.textContent = refusalText(err);
      }
    }
  }

  focusFirst() {}
}
