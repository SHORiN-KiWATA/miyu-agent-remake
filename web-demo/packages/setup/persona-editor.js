// @ts-check
//! 人格的详情（蓝图 `web.md`「人格、预设、工作区」第 6 条）：在列表里原地展开，名字、说明、人设、示范对话、角色扮演提示
//! （2026-10-08 项目主人：来自哪一层、以谁为底、编号都不露；说明人自己写，出厂的那句能改）。改完点「保存」一起发一条 `persona.set`（写一半不生效），提示词带
//! `persona.read` 给的版本；「取消」收起、丢掉没存的。删除照核心的 `remove`：改过的出厂「恢复出厂」、自己建的「删除」，点两次才删。
//! 别处改过了（`persona_conflict`）写一句、给「重新读」；写错的照核心给人看的那一句（`data.message`）。

import { h, replace } from '../../src/lib/dom.js';
import { field, head, area, twoClick, foldable } from './form.js';
import { Pairs } from './pairs.js';
import { personaSave, halfPair } from './model.js';

/**
 * @typedef {import('./model.js').PersonaDraft} PersonaDraft
 * @typedef {{saved: () => void, removed: (remains: boolean) => void, close: () => void}} Hooks
 */

/** 核心拒了的给人看的那一句：写错的照 `data.message`（照连接的语言），别的照原话。 @param {any} err */
export const refusalText = (err) => err?.data?.message ?? err?.message ?? String(err);

export class PersonaEditor {
  /** @param {any} ctx @param {import('./form.js').Kit} kit @param {string} id @param {Hooks} hooks */
  constructor(ctx, kit, id, hooks) {
    this.ctx = ctx;
    this.kit = kit;
    this.id = id;
    this.hooks = hooks;
    this.t = (/** @type {string} */ key, /** @type {any} */ fields) => ctx.text(key, fields);
    this.title = h('h4', id);
    this.body = h('div.setup-editor', h('p.setup-empty', this.t('page.loading')));
    this.fold = foldable(this.body);
    this.el = h('div.setup-card.is-open', { 'data-set-dismiss': '' }, head(this.title, this.t('edit.collapse'), () => this.tryClose()), this.fold.wrap);
    // `Esc`：没改过的收起；改了没存的留着（长的字一按就丢太亏），提示先存或者取消
    this.el.addEventListener('set-dismiss', () => this.tryClose());
    /** @type {PersonaDraft|null} */
    this.before = null;
    /** @type {string|null} */
    this.remove = null;
  }

  /** 读这个人格和三份提示词（`persona.read`），画出来；读不到写原因。 */
  async load() {
    const read = (prompt) => this.ctx.core.request('persona.read', { persona: this.id, prompt });
    try {
      const [got, persona, reminders, examples] = await Promise.all([
        this.ctx.core.request('persona.get', { persona: this.id }), read('persona'), read('reminders'), read('examples'),
      ]);
      this.remove = got?.remove ?? null;
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
    replace(this.body,
      field(t('edit.name'), this.name),
      field(t('edit.summary'), this.summary),
      field(t('edit.persona'), this.persona),
      field(t('edit.examples'), this.pairs.el),
      field(t('edit.reminders'), this.reminders),
      this.note,
      h('div.setup-foot', remove, h('span.setup-grow'), this.kit.button(t('edit.cancel'), {}, () => this.collapse()), this.saveBtn));
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

  /** 改了没存的不收起，提示一句；没改的收起。 */
  tryClose() {
    if (this.dirty()) {
      this.say(this.t('edit.unsaved'));
      this.saveBtn?.focus();
      return;
    }
    this.collapse();
  }

  /** 展开（放进页面以后）。 */
  expand() {
    this.fold.open();
  }

  /** 收起：动画走完再交给列表换回那一块。 */
  collapse() {
    this.fold.close(() => this.hooks.close());
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
