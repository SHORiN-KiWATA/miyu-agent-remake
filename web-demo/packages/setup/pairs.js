// @ts-check
//! 示范对话一对一对地写（蓝图 `web.md`「人格、预设、工作区」第 6 条；2026-10-08 项目主人：照旧版一对 user/assistant 地加，不让人手写
//! 格式）：一对两行，「你」和人格的名字各一个多行框（不写「她」，人格不一定是女性），右边 ✕ 删这一对；下面「＋ 加一对」。

import { h, icon } from '../../src/lib/dom.js';
import { area } from './form.js';

/**
 * @typedef {import('./model.js').Pair} Pair
 * @typedef {{you: string, them: () => string, add: string, drop: string, userHint: string, replyHint: string}} Words
 */

export class Pairs {
  /** @param {Pair[]} pairs @param {Words} words @param {() => void} changed 加了、删了、改了一个字 */
  constructor(pairs, words, changed) {
    this.words = words;
    this.changed = changed;
    /** @type {{user: HTMLTextAreaElement, assistant: HTMLTextAreaElement, who: HTMLElement, el: HTMLElement}[]} */
    this.rows = [];
    this.list = h('div.setup-pairs');
    for (const p of pairs) this.add(p);
    const more = h('button.setup-add', { type: 'button', onclick: () => { this.add({ user: '', assistant: '' }).user.focus(); this.changed(); } }, icon('plus'), h('span', words.add));
    this.el = h('div', this.list, more);
  }

  /** 末尾加一对。 @param {Pair} p */
  add(p) {
    const user = area(p.user, this.words.userHint, 1);
    const assistant = area(p.assistant, this.words.replyHint, 1);
    const who = h('span.setup-who', this.words.them());
    /** @type {{user: HTMLTextAreaElement, assistant: HTMLTextAreaElement, who: HTMLElement, el: HTMLElement}} */
    const r = { user, assistant, who, el: h('div') };
    const drop = h('button.setup-drop', { type: 'button', title: this.words.drop, 'aria-label': this.words.drop, onclick: () => this.drop(r) }, icon('x'));
    r.el = h('div.setup-pair',
      h('div.setup-turn', h('span.setup-who', this.words.you), user, drop),
      h('div.setup-turn', who, assistant, h('span')));
    for (const a of [user, assistant]) a.addEventListener('input', () => { r.el.classList.remove('is-bad'); this.changed(); });
    this.rows.push(r);
    this.list.append(r.el);
    return r;
  }

  /** @param {{el: HTMLElement}} r */
  drop(r) {
    this.rows.splice(this.rows.indexOf(/** @type {any} */ (r)), 1);
    r.el.remove();
    this.changed();
  }

  /** @returns {Pair[]} */
  value() {
    return this.rows.map((r) => ({ user: r.user.value, assistant: r.assistant.value }));
  }

  /** 名字改了：回的那一头跟着写新名字。 @param {string} name */
  rename(name) {
    for (const r of this.rows) r.who.textContent = name;
  }

  /** 只写了一头的那一对标出来，焦点放到空着的那一头。 @param {number} i */
  mark(i) {
    const r = this.rows[i];
    if (!r) return;
    r.el.classList.add('is-bad');
    (r.user.value.trim() ? r.assistant : r.user).focus();
  }
}
