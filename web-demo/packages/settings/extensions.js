// @ts-check
//! 「软件包」页核心那一段里核心拉起的扩展（蓝图 `web.md`「设置页」第 13 条；核心 9-4 上、9-4 补、9-4 下上，`extensions.md`）：一个扩展一组，
//! 组名是它的名字；「运行」一行写现在怎样（关着、正在启动、在运行、几秒后再试、停了为什么），右边开关，在运行的多一个「重启」，停了的
//! 能点开看标准错误的最后几行；「权限」一行写它要哪些能力，还没批的标出来。开的那一下还有没批的：这一组里展开一块「开启前要批准这些
//! 权限」，一条一条写名字和一句说明，「批准并开启」就是 `extension.enable` 带上 `approve`（2026-10-08 项目主人照推荐定：在开的那一下批）。
//! 状态照 `subscribe {"stream": "extensions"}` 的推送当场跟上（`extension.changed` 整项换，掉队重订）。

import { h, replace } from '../../src/lib/dom.js';
import { toggle } from './rows.js';

/**
 * @typedef {{id: string, name: string, summary: string|null}} Capability
 * @typedef {{package: string, name: string, on: boolean, state: string, failures?: number, retry_in?: number, reason?: string, stderr?: string[]|string,
 *   capabilities?: Capability[], unapproved?: string[]}} Entry
 */

export class Extensions {
  /** @param {any} dialog 设置页的弹窗（`dialog.js`） */
  constructor(dialog) {
    this.dialog = dialog;
    this.ctx = dialog.ctx;
    /** @type {Map<string, Entry>} */
    this.entries = new Map();
    /** 订阅着（弹窗开着时订一次，关了退订） */
    this.live = false;
    /** 要批准的那一块开着的扩展 */
    this.asking = new Set();
    /** 点开了标准错误的扩展 */
    this.showing = new Set();
    /** 编号 → 这一组画在哪（推送来了只重画这一组） @type {Map<string, HTMLElement>} */
    this.slots = new Map();
    /** 编号 → 这个包自己的设置项那几行（接在后面） @type {Map<string, Node[]>} */
    this.rows = new Map();
  }

  /** 订阅扩展的状态：回应就是全部，之后照推送换。 */
  async start() {
    if (this.live) return;
    this.live = true;
    try {
      const got = await this.ctx.core.request('subscribe', { stream: 'extensions' });
      this.entries = new Map((got?.extensions ?? []).map((e) => [e.package, e]));
    } catch {
      this.live = false;
    }
  }

  /** 弹窗关了：退订。 */
  stop() {
    if (!this.live) return;
    this.live = false;
    this.ctx.core.request('unsubscribe', { stream: 'extensions' }).catch(() => {});
  }

  /** 推送：一个扩展变了整项换、重画那一组；掉了队重订。交回认不认得这条。 @param {string} method @param {any} params */
  push(method, params) {
    if (method === 'extension.changed' && params?.entry?.package) {
      this.entries.set(params.entry.package, params.entry);
      this.redraw(params.entry.package);
      return true;
    }
    if (method === 'resync' && params?.stream === 'extensions') {
      this.live = false;
      this.start().then(() => { for (const id of this.slots.keys()) this.redraw(id); });
      return true;
    }
    return false;
  }

  /** 扩展的编号。 */
  ids() {
    return [...this.entries.keys()];
  }

  /**
   * 一个扩展的那一组：组名、「运行」「权限」两行、要批准时的那一块，再接这个包自己的设置项（`rows`，照 `config.schema` 那一组）。
   * @param {string} id @param {Node[]} rows
   */
  block(id, rows) {
    const slot = h('section.set-group.set-ext');
    this.slots.set(id, slot);
    this.rows.set(id, rows);
    this.fill(id, slot);
    return slot;
  }

  /** @param {string} id */
  redraw(id) {
    const slot = this.slots.get(id);
    if (slot?.isConnected) this.fill(id, slot);
  }

  /** @param {string} id @param {HTMLElement} slot */
  fill(id, slot) {
    const e = this.entries.get(id);
    if (!e) return;
    const t = (key, fields) => this.ctx.text(key, fields);
    const caps = e.capabilities ?? [];
    const unapproved = new Set(e.unapproved ?? []);
    const stderr = Array.isArray(e.stderr) ? e.stderr.join('\n') : e.stderr ?? '';
    const restart = e.state === 'running'
      ? h('button.set-btn', { type: 'button', onclick: () => this.act('extension.restart', { package: id }) }, t('ext.restart')) : null;
    const peek = e.on && e.state === 'stopped' && stderr
      ? h('button.set-link', { type: 'button', onclick: () => { this.toggleSet(this.showing, id); this.redraw(id); } }, t(this.showing.has(id) ? 'ext.hide_output' : 'ext.show_output')) : null;
    const run = h('div.set-row',
      h('div.set-text', h('div.set-name', h('span', t('ext.run'))),
        h(`p.set-desc${e.on && e.state === 'stopped' ? '.is-error' : ''}`, stateText(t, e)), peek),
      h('div.set-control', restart, toggle(e.on, (on) => (on ? this.turnOn(e) : this.act('extension.disable', { package: id })))));
    const output = this.showing.has(id) && stderr ? h('pre.set-ext-output', stderr) : null;
    const perms = caps.length ? h('div.set-row',
      h('div.set-text', h('div.set-name', h('span', t('ext.perms'))),
        h('p.set-desc', caps.map((c, i) => [i ? '、' : '', h(`span${unapproved.has(c.id) ? '.set-ext-unapproved' : ''}`, { title: c.summary ?? null }, c.name)])),
        unapproved.size ? h('p.set-problem.is-warn', t('ext.unapproved')) : null)) : null;
    const ask = this.asking.has(id) || (e.on && e.reason === 'needs_approval') ? this.approval(e, caps, unapproved) : null;
    replace(slot, h('h3.set-group-name', e.name), h('div.set-rows', run, output, perms, ask, ...(this.rows.get(id) ?? [])));
  }

  /** 要批准的那一块：没批的一条一条写名字、一句说明；「批准并开启」「取消」。 @param {Entry} e @param {Capability[]} caps @param {Set<string>} unapproved */
  approval(e, caps, unapproved) {
    const t = (key) => this.ctx.text(key);
    const list = caps.filter((c) => unapproved.has(c.id));
    return h('div.set-ext-ask',
      h('p.set-ext-ask-title', t('ext.ask')),
      h('ul', list.map((c) => h('li', h('b', c.name), c.summary ? h('span', c.summary) : null))),
      h('div.set-form-buttons',
        h('button.set-btn', { type: 'button', onclick: () => { this.asking.delete(e.package); if (e.on && e.reason === 'needs_approval') this.act('extension.disable', { package: e.package }); else this.redraw(e.package); } }, t('ext.cancel')),
        h('button.set-btn.is-primary', { type: 'button', onclick: () => { this.asking.delete(e.package); this.act('extension.enable', { package: e.package, approve: list.map((c) => c.id) }); } }, t('ext.approve'))));
  }

  /** 开：还有没批的先展开要批准的那一块，不发；都批过的直接开。 @param {Entry} e */
  turnOn(e) {
    if ((e.unapproved ?? []).length) {
      this.asking.add(e.package);
      this.redraw(e.package);
      return;
    }
    this.act('extension.enable', { package: e.package });
  }

  /** 开、关、重启：回应是那一个，整项换；拒了的照原话提示（核心拒「要先批」的展开那一块）。 @param {string} method @param {any} params */
  async act(method, params) {
    try {
      const got = await this.ctx.core.request(method, params);
      if (got?.package) this.entries.set(got.package, got);
    } catch (err) {
      if (err?.reason === 'needs_approval') this.asking.add(params.package);
      else this.dialog.toast(err?.message ?? String(err));
    }
    this.redraw(params.package);
  }

  /** @param {Set<string>} set @param {string} id */
  toggleSet(set, id) {
    if (set.has(id)) set.delete(id);
    else set.add(id);
  }
}

/** 「运行」那一行的说明：现在怎样。关着的只写「关着」（核心关了以后还留着上一次停下的原因，人关的不用再说为什么）。 @param {(key: string, fields?: any) => string} t @param {Entry} e */
export function stateText(t, e) {
  if (!e.on) return t('ext.state.off');
  if (e.state === 'running') return t('ext.state.running');
  if (e.state === 'starting') return t('ext.state.starting');
  if (e.state === 'waiting') return t('ext.state.waiting', { seconds: Math.max(1, Math.ceil((e.retry_in ?? 0) / 1000)), failures: e.failures ?? 0 });
  if (e.state === 'stopped') {
    const why = e.reason ? t(`ext.reason.${e.reason}`) : '';
    return why && !why.startsWith('ext.') ? why : t('ext.state.stopped');
  }
  return t('ext.state.off');
}
