// @ts-check
//! 预设的详情（蓝图 `web.md`「人格、预设、工作区」第 6 条）：浮起来的卡片里（`float.js`）——名字、功能开关（2026-10-08 项目主人：预设不要说明，
//! 「没什么意义」；不带默认人格，人格、预设互不引用）。都是点了当场存（`preset.set` 只带改的那一项，不带 `expect`）：名字回车、离开时存，开关点了就存；功能照核心给的
//! 名字（`software`，照连接的语言），没装的写「没安装」。删除照核心的 `remove`，同人格。

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
    const software = Array.isArray(got.software) ? got.software : [];
    const features = h('div.setup-features', software.map((s) => h('div.setup-feature', { title: s.summary ?? null },
      h('span', s.name ?? s.id),
      s.installed === false
        ? h('span.setup-missing', t('edit.not_installed'))
        : this.kit.toggle(!!s.on, (on) => this.set([{ key: `software.${s.id}`, value: on }])))));
    this.note = h('p.setup-error', { hidden: true });
    const remove = got.remove ? twoClick(this.kit, t(`edit.${got.remove}`), t(`edit.${got.remove}_again`), () => this.drop()) : null;
    replace(this.body,
      field(t('edit.name'), name),
      field(t('edit.features'), software.length ? features : h('p.setup-empty', t('edit.no_features'))),
      this.note,
      remove ? h('div.setup-foot', remove) : null);
  }

  /** 改一项：成了照回应重画（回应同 `preset.get`），拒了写原因。 @param {any[]} changes */
  async set(changes) {
    try {
      const got = await this.ctx.core.request('preset.set', { preset: this.id, changes });
      this.draw(got);
      this.hooks.saved();
    } catch (err) {
      if (this.note) {
        this.note.hidden = false;
        this.note.textContent = refusalText(err);
      }
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
