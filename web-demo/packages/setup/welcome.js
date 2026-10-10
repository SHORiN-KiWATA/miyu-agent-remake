// @ts-check
//! 第一次引导里人格、预设那两屏（蓝图 `web.md`「第一次引导」第 8、9 条），经服务 `personas` 交给软件包 `welcome`：
//! - 人格：和设置页的人格详情一样几格（头像、名称、说明、人设、示范对话、角色扮演提示），名称必填；「下一步」一条 `persona.set` 新建
//!   （不写编号，头像一起），再设成默认人格。默认人格已经指着一个在的人格（中途关了重来）：读它填好，只存改了的。
//! - 预设：一个预设一张卡片，写开了、关了哪些功能；最后一张「自定义」原地展开名称和功能开关。「下一步」设成默认预设（自定义的先建）。
//! 每一屏交 `{title, sub, body, next: {ready, run}, focus}`，上一步、下一步的按钮由引导画；`run` 交回出错的字，成了是 `null`。

import { h, replace } from '../../src/lib/dom.js';
import { field, area } from './form.js';
import { Pairs } from './pairs.js';
import { personaSave, halfPair, personaName, presetName } from './model.js';
import { shrink, store } from './avatars.js';
import { refusalText } from './persona-editor.js';

/**
 * @typedef {{ready: () => void, mascot?: {hop: () => void}|null}} Host 主按钮能不能点变了；吉祥物（有的话）
 * @typedef {import('./model.js').PersonaDraft} PersonaDraft
 */

/** 新建时的「之前」：都是空的（`personaSave` 照它算出全部要写的） @returns {PersonaDraft} */
const blank = () => ({ name: '', summary: '', persona: '', reminders: '', pairs: [], versions: { persona: null, reminders: null, examples: null } });

export class PersonaStep {
  /**
   * @param {any} ctx `setup` 包的 ctx @param {import('./form.js').Kit} kit @param {import('./catalog.js').Catalog} catalog @param {Host} host
   */
  constructor(ctx, kit, catalog, host) {
    this.ctx = ctx;
    this.kit = kit;
    this.catalog = catalog;
    this.host = host;
    this.t = (/** @type {string} */ key, /** @type {any} */ fields) => ctx.text(key, fields);
    /** 改的是哪个人格（中途关了重来的那一个），新建的是 `null` */
    this.id = /** @type {string|null} */ (null);
    /** @type {PersonaDraft} */
    this.before = blank();
    /** 选了还没存的头像：blob 的哈希、预览的地址 */
    this.avatar = /** @type {{hash: string, url: string}|null} */ (null);
    /** 改的那个人格原来的头像版本 */
    this.version = /** @type {string|null} */ (null);
    this.avatarBox = h('div.setup-avatar-box');
    this.note = h('p.setup-error', { hidden: true });
  }

  /** 默认人格指着一个在的人格的，读它（改它，不再建一个）。 */
  async load() {
    await this.catalog.load();
    const id = this.catalog.personaDefault;
    if (!id || !this.catalog.personas?.some((p) => p.persona === id && !p.problem)) return;
    const read = (/** @type {string} */ prompt) => this.ctx.core.request('persona.read', { persona: id, prompt });
    try {
      const [got, persona, reminders, examples] = await Promise.all([this.ctx.core.request('persona.get', { persona: id }), read('persona'), read('reminders'), read('examples')]);
      this.id = id;
      this.version = got?.avatar ?? null;
      this.before = {
        name: typeof got?.name === 'string' ? got.name : '',
        summary: typeof got?.summary === 'string' ? got.summary : '',
        persona: persona?.text ?? '',
        reminders: reminders?.text ?? '',
        pairs: Array.isArray(examples?.pairs) ? examples.pairs : [],
        versions: { persona: persona?.version ?? null, reminders: reminders?.version ?? null, examples: examples?.version ?? null },
      };
    } catch {
      // 读不到的当没有，新建一个
    }
  }

  /** 这一屏。 */
  screen() {
    const t = this.t;
    const d = this.before;
    this.name = this.kit.field(d.name, t('welcome.name_hint'));
    this.summary = this.kit.field(d.summary, t('edit.summary_hint'));
    this.persona = area(d.persona, t('edit.persona_hint'), 4);
    this.reminders = area(d.reminders, t('edit.reminders_hint'), 2);
    const them = () => this.name?.value.trim() || t('edit.them');
    this.pairs = new Pairs(d.pairs, { you: t('edit.you'), them, add: t('edit.add_pair'), drop: t('edit.drop_pair'), userHint: t('edit.user_hint'), replyHint: t('edit.reply_hint') }, () => this.say(''));
    this.name.addEventListener('input', () => {
      this.pairs?.rename(them());
      this.paintAvatar();
      this.host.ready();
    });
    this.paintAvatar();
    const body = h('div.setup-editor.is-welcome',
      field(t('edit.avatar'), this.avatarBox),
      field(t('edit.name'), this.name),
      field(t('edit.summary'), this.summary),
      field(t('edit.persona'), this.persona),
      field(t('edit.examples'), this.pairs.el),
      field(t('edit.reminders'), this.reminders),
      this.note);
    return {
      title: t(this.id ? 'welcome.persona_edit' : 'welcome.persona_title'),
      sub: t('welcome.persona_sub'),
      body,
      next: { ready: () => !!this.name?.value.trim(), run: () => this.save() },
      focus: this.name,
    };
  }

  /** 头像那一格：圆的图（还没选的画名字的第一个字），「选择图片」。 */
  paintAvatar() {
    const name = this.name?.value.trim() || this.before.name;
    const url = this.avatar?.url ?? (this.id ? this.catalog.avatars.url(this.id, this.version) : null);
    const pick = /** @type {HTMLInputElement} */ (h('input', { type: 'file', accept: 'image/png,image/jpeg,image/webp,image/*', hidden: true }));
    pick.addEventListener('change', () => {
      const file = pick.files?.[0];
      pick.value = '';
      if (file) this.choose(file);
    });
    replace(this.avatarBox,
      h('span.setup-face.is-persona.is-big', { 'aria-hidden': 'true' }, url ? h('img', { src: url, alt: '' }) : [...name][0] ?? ''),
      this.kit.button(this.t(url ? 'edit.avatar_change' : 'welcome.avatar_pick'), {}, () => pick.click()),
      pick);
  }

  /** 选了一张：缩小、传成 blob，存的时候一起交。 @param {File} file */
  async choose(file) {
    const cfg = this.ctx.config;
    try {
      const blob = await shrink(file, cfg.avatar_side);
      const hash = await store(this.ctx.core, blob, { chunk: cfg.avatar_chunk_bytes, tries: cfg.avatar_tries });
      if (this.avatar) URL.revokeObjectURL(this.avatar.url);
      this.avatar = { hash, url: URL.createObjectURL(blob) };
      this.say('');
      this.paintAvatar();
    } catch (err) {
      this.say(refusalText(err));
    }
  }

  /** @param {string} text */
  say(text) {
    this.note.hidden = !text;
    this.note.textContent = text;
  }

  /** 存：新建或者改，再设成默认人格。 */
  async save() {
    const t = this.t;
    const pairs = this.pairs?.value() ?? [];
    const half = halfPair(pairs);
    if (half >= 0) {
      this.pairs?.mark(half);
      return t('edit.half_pair', { n: half + 1 });
    }
    const now = { ...this.before, name: this.name?.value ?? '', summary: this.summary?.value ?? '', persona: this.persona?.value ?? '', reminders: this.reminders?.value ?? '', pairs };
    const payload = personaSave(this.before, now) ?? {};
    const avatar = this.avatar ? { avatar: { blob: this.avatar.hash } } : {};
    try {
      let id = this.id;
      if (!id || Object.keys(payload).length || this.avatar) {
        const got = await this.ctx.core.request('persona.set', { ...(id ? { persona: id } : {}), ...payload, ...avatar });
        id = id ?? got?.persona ?? null;
      }
      if (!id) return t('welcome.save_failed');
      if (this.catalog.personaDefault !== id) await this.ctx.core.request('config.set', { layer: 'personal', changes: [{ key: 'persona.default', value: id }] });
      this.id = id;
      this.before = now;
      this.avatar = null;
      this.host.mascot?.hop();
      await this.catalog.load();
      return null;
    } catch (err) {
      return refusalText(err);
    }
  }

  /** 存好了的人格叫什么（完成那一屏写）。 */
  label() {
    const p = this.catalog.personas?.find((x) => x.persona === this.id);
    return p ? personaName(p) : this.name?.value.trim() ?? '';
  }
}

export class PresetStep {
  /**
   * @param {any} ctx @param {import('./form.js').Kit} kit @param {import('./catalog.js').Catalog} catalog @param {Host} host
   */
  constructor(ctx, kit, catalog, host) {
    this.ctx = ctx;
    this.kit = kit;
    this.catalog = catalog;
    this.host = host;
    this.t = (/** @type {string} */ key, /** @type {any} */ fields) => ctx.text(key, fields);
    /** 每个预设开了哪些功能（`preset.get` 的 `features`，没装的不列） @type {Map<string, any[]>} */
    this.features = new Map();
    /** 选了哪一张：预设的编号，或者 `custom` */
    this.chosen = 'full';
    /** 自定义：名字、每个功能开不开 */
    this.customOn = /** @type {Map<string, boolean>} */ (new Map());
    /** 装了的全部功能（自定义那一块列的） @type {any[]} */
    this.all = [];
    this.id = /** @type {string|null} */ (null);
  }

  async load() {
    await this.catalog.load();
    const list = (this.catalog.presets ?? []).filter((p) => !p.problem);
    const got = await Promise.all(list.map((p) => this.ctx.core.request('preset.get', { preset: p.preset }).catch(() => null)));
    list.forEach((p, i) => this.features.set(p.preset, (got[i]?.features ?? []).filter((/** @type {any} */ f) => f.installed !== false)));
    const def = this.catalog.presetDefault;
    this.chosen = def && this.features.has(def) ? def : this.features.has('full') ? 'full' : list[0]?.preset ?? 'custom';
    // 自定义先照全部开着
    const all = [...this.features.values()].reduce((a, f) => (f.length > a.length ? f : a), /** @type {any[]} */ ([]));
    this.customOn = new Map(all.map((f) => [f.id, true]));
    this.all = all;
  }

  screen() {
    const t = this.t;
    const box = h('div.setup-choices');
    this.customName = this.kit.field('', t('welcome.name_hint'));
    this.customName.addEventListener('input', () => this.host.ready());
    const draw = () => {
      const cards = (this.catalog.presets ?? []).filter((p) => this.features.has(p.preset)).map((p) => {
        const on = this.chosen === p.preset;
        return h(`button.setup-choice${on ? '.is-on' : ''}`, { type: 'button', onclick: () => { this.chosen = p.preset; draw(); this.host.ready(); } },
          h('span.setup-choice-name', presetName(p)),
          h('span.setup-chips', (this.features.get(p.preset) ?? []).map((f) => h(`span.setup-chip${f.on ? '' : '.is-off'}`, f.name ?? f.id))),
          on ? h('span.setup-choice-check', '✓') : null);
      });
      const custom = this.chosen === 'custom';
      const head = h(`button.setup-choice${custom ? '.is-on' : ''}`, { type: 'button', onclick: () => { if (custom) return; this.chosen = 'custom'; draw(); this.host.ready(); this.customName?.focus(); } },
        h('span.setup-choice-name', t('welcome.custom')), custom ? null : h('span.setup-choice-hint', t('welcome.custom_hint')), custom ? h('span.setup-choice-check', '✓') : null);
      const open = custom ? h('div.setup-choice-open',
        field(t('edit.name'), /** @type {HTMLElement} */ (this.customName)),
        h('div.setup-features', (this.all ?? []).map((f) => h('div.setup-feature', h('span.setup-feature-name', { title: f.summary ?? null }, f.name ?? f.id),
          this.kit.toggle(this.customOn.get(f.id) !== false, (v) => { this.customOn.set(f.id, v); return true; }))))) : null;
      replace(box, cards, h(`div.setup-choice-wrap${custom ? '.is-on' : ''}`, head, open));
    };
    draw();
    return {
      title: t('welcome.preset_title'),
      sub: t('welcome.preset_sub'),
      body: box,
      next: { ready: () => this.chosen !== 'custom' || !!this.customName?.value.trim(), run: () => this.save() },
      focus: /** @type {HTMLElement|null} */ (box.querySelector('.setup-choice.is-on')),
    };
  }

  /** 存：自定义的先建；和现在的默认不一样的设成默认预设。 */
  async save() {
    try {
      let id = this.chosen;
      if (id === 'custom') {
        const changes = [{ key: 'preset.name', value: this.customName?.value.trim() }];
        for (const [f, on] of this.customOn) if (!on) changes.push({ key: `features.${f}`, value: false });
        const got = await this.ctx.core.request('preset.set', { changes });
        id = got?.preset;
        if (!id) return this.t('welcome.save_failed');
        // 建好了：回到这一屏时它是一张卡片，不再建一个
        this.features.set(id, (got.features ?? []).filter((/** @type {any} */ f) => f.installed !== false));
        this.chosen = id;
      }
      if (this.catalog.presetDefault !== id) await this.ctx.core.request('config.set', { layer: 'personal', changes: [{ key: 'preset.default', value: id }] });
      this.id = id;
      await this.catalog.load();
      return null;
    } catch (err) {
      return refusalText(err);
    }
  }

  /** 选的预设叫什么（完成那一屏写）。 */
  label() {
    const p = this.catalog.presets?.find((x) => x.preset === this.id);
    return p ? presetName(p) : this.id ?? '';
  }
}
