// @ts-check
//! 人格的详情（蓝图 `web.md`「人格、预设、工作区」第 6 条）：浮起来的卡片里（`float.js`，列表那一页 `page.js` 开它），名字、说明、人设、示范对话、角色扮演提示
//! （2026-10-08 项目主人：来自哪一层、以谁为底、编号都不露；说明人自己写，出厂的那句能改）。改完点「保存」一起发一条 `persona.set`（写一半不生效），提示词带
//! `persona.read` 给的版本；「取消」关掉卡片、丢掉没存的。删除照核心的 `remove`：改过的出厂「恢复出厂」、自己建的「删除」，点两次才删。
//! 别处改过了（`persona_conflict`）写一句、给「重新读」；写错的照核心给人看的那一句（`data.message`）。
//! 最上面三行是外观（头像、主题色、背景图，`appearance.js`）：换、删当场存，不等「保存」。

import { h, replace } from '../../src/lib/dom.js';
import { field, area, twoClick } from './form.js';
import { Pairs } from './pairs.js';
import { personaSave, halfPair } from './model.js';
import { Appearance } from './appearance.js';

/**
 * @typedef {import('./model.js').PersonaDraft} PersonaDraft
 * @typedef {{saved: () => void, removed: (remains: boolean) => void, close: () => void}} Hooks
 */

/** 核心拒了的给人看的那一句：写错的照 `data.message`（照连接的语言），别的照原话。 @param {any} err */
export const refusalText = (err) => err?.data?.message ?? err?.message ?? String(err);

export class PersonaEditor {
  /**
   * @param {any} ctx @param {import('./form.js').Kit} kit @param {import('./catalog.js').Catalog} catalog @param {string} id @param {Hooks} hooks
   * @param {{name?: string}} [look] 列表里那一块写的（读完以前卡片头上先写它的名字，不露编号）
   */
  constructor(ctx, kit, catalog, id, hooks, look = {}) {
    this.ctx = ctx;
    this.kit = kit;
    this.catalog = catalog;
    this.id = id;
    this.hooks = hooks;
    this.t = (/** @type {string} */ key, /** @type {any} */ fields) => ctx.text(key, fields);
    /** 卡片头上的名字 */
    this.title = h('span', look.name ?? id);
    /** 卡片里的内容 */
    this.body = h('div.setup-editor', h('p.setup-empty', this.t('page.loading')));
    /** @type {PersonaDraft|null} */
    this.before = null;
    /** @type {string|null} */
    this.remove = null;
    /** 外观那三行 */
    this.look = new Appearance(ctx, kit, catalog, id, {
      saved: () => this.hooks.saved(),
      say: (text) => this.say(text),
      name: () => this.name?.value.trim() || this.before?.name || '',
    });
  }

  /** 读这个人格和三份提示词（`persona.read`），画出来；读不到写原因。 */
  async load() {
    const read = (prompt) => this.ctx.core.request('persona.read', { persona: this.id, prompt });
    try {
      const [got, persona, reminders, examples] = await Promise.all([
        this.ctx.core.request('persona.get', { persona: this.id }), read('persona'), read('reminders'), read('examples'),
      ]);
      this.remove = got?.remove ?? null;
      this.look.set(got);
      this.before = {
        name: typeof got?.name === 'string' ? got.name : '',
        summary: typeof got?.summary === 'string' ? got.summary : '',
        persona: persona?.text ?? '',
        reminders: reminders?.text ?? '',
        pairs: Array.isArray(examples?.pairs) ? examples.pairs : [],
        versions: { persona: persona?.version ?? null, reminders: reminders?.version ?? null, examples: examples?.version ?? null },
      };
      this.draw(this.before);
    } catch (err) {
      replace(this.body, h('p.setup-error', refusalText(err)));
    }
  }

  /** @param {PersonaDraft} d */
  draw(d) {
    const t = this.t;
    this.title.textContent = d.name || this.id;
    this.name = this.kit.field(d.name, '');
    this.summary = this.kit.field(d.summary, t('edit.summary_hint'));
    this.persona = area(d.persona, t('edit.persona_hint'), 4);
    this.reminders = area(d.reminders, t('edit.reminders_hint'), 2);
    const them = () => this.name?.value.trim() || t('edit.them');
    this.pairs = new Pairs(d.pairs, { you: t('edit.you'), them, add: t('edit.add_pair'), drop: t('edit.drop_pair'), userHint: t('edit.user_hint'), replyHint: t('edit.reply_hint') }, () => this.sync());
    this.name.addEventListener('input', () => { this.pairs?.rename(them()); this.sync(); });
    for (const a of [this.summary, this.persona, this.reminders]) a.addEventListener('input', () => this.sync());
    this.note = h('p.setup-error', { hidden: true });
    this.saveBtn = this.kit.button(t('edit.save'), { primary: true }, () => this.save());
    const remove = this.remove ? twoClick(this.kit, t(`edit.${this.remove}`), t(`edit.${this.remove}_again`), () => this.drop()) : null;
    this.look.paint();
    replace(this.body,
      field(t('edit.avatar'), this.look.avatarBox),
      field(t('edit.seed'), this.look.seedBox),
      field(t('edit.background'), this.look.backgroundBox),
      field(t('edit.name'), this.name),
      field(t('edit.summary'), this.summary),
      field(t('edit.persona'), this.persona),
      field(t('edit.examples'), this.pairs.el),
      field(t('edit.reminders'), this.reminders),
      this.note,
      h('div.setup-foot', remove, h('span.setup-grow'), this.kit.button(t('edit.cancel'), {}, () => this.hooks.close()), this.saveBtn));
    this.sync();
  }

  /** 编辑器里现在写的。 @returns {PersonaDraft} */
  current() {
    const b = /** @type {PersonaDraft} */ (this.before);
    return { ...b, name: this.name?.value ?? b.name, summary: this.summary?.value ?? b.summary, persona: this.persona?.value ?? b.persona, reminders: this.reminders?.value ?? b.reminders, pairs: this.pairs?.value() ?? b.pairs };
  }

  /** 改了没存。 */
  dirty() {
    return !!this.before && personaSave(this.before, this.current()) != null;
  }

  /** 「存」：改了、名字不空才点得了。 */
  sync() {
    if (this.saveBtn) this.saveBtn.disabled = !this.dirty() || !this.name?.value.trim();
  }

  /** @param {string} text @param {HTMLElement[]} [more] */
  say(text, more = []) {
    if (!this.note) return;
    this.note.hidden = !text;
    replace(this.note, h('span', text), ...more);
  }

  /** 点外面、`Esc`、✕：改了没存的不关（长的字一按就丢太亏），提示先存或者取消；没改的关掉。 */
  tryClose() {
    if (this.dirty()) {
      this.say(this.t('edit.unsaved'));
      this.saveBtn?.focus();
      return;
    }
    this.hooks.close();
  }

  async save() {
    if (!this.before || !this.saveBtn) return;
    const half = halfPair(this.pairs?.value() ?? []);
    if (half >= 0) {
      this.say(this.t('edit.half_pair', { n: half + 1 }));
      this.pairs?.mark(half);
      return;
    }
    const payload = personaSave(this.before, this.current());
    if (!payload) return;
    this.saveBtn.disabled = true;
    try {
      await this.ctx.core.request('persona.set', { persona: this.id, ...payload });
    } catch (err) {
      this.sync();
      if (err?.reason === 'persona_conflict') {
        this.say(this.t('edit.conflict'), [h('button.setup-link', { type: 'button', onclick: () => this.load() }, this.t('edit.reload'))]);
      } else {
        this.say(refusalText(err));
      }
      return;
    }
    this.kit.toast(this.t('edit.saved'));
    await this.load();
    this.hooks.saved();
  }

  async drop() {
    try {
      const got = await this.ctx.core.request('persona.delete', { persona: this.id });
      this.kit.toast(this.t(got?.remains ? 'edit.restored' : 'edit.deleted'));
      this.hooks.removed(!!got?.remains);
    } catch (err) {
      this.say(refusalText(err));
    }
  }

  /** 新建完接着写：焦点放到人设。 */
  focusFirst() {
    this.persona?.focus();
  }
}
