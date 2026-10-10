// @ts-check
//! 接一家供应商的几屏（蓝图 `web.md`「第一次引导」第 5–7、12 条）：选一家（常用的几家、本机的服务、自定义、搜索整份目录）→ 一家
//! （密钥、测试连接、选模型）或者自定义（地址、接口、密钥）。第一次引导和设置页的「＋ 添加供应商」用的是这同一段：屏画在哪、上一步
//! 下一步的按钮画成什么样由宿主定（`Host`），这里只交每一屏的标题、内容和主按钮。纯逻辑在 `logic.js`。
//!
//! 测的时候叫吉祥物敲电脑（宿主交来的 `mascot`），成了收起来跳一下，没成耷拉一下耳朵。

import { h, icon, replace } from '../../../src/lib/dom.js';
import { logoEl } from '../../../src/lib/logo.js';
import { secretName } from '../model.js';
import { select } from '../rows.js';
import { providerRows, testParams, setupChanges, noPools, modelOrder, filterNames, failureOf, validUrl, cleanUrl } from './logic.js';

/** 模型最多列几个（再多的搜） */
const MAX_MODELS = 200;
/** 搜索整份目录：停下多久再搜（毫秒）、最多几家 */
const SEARCH_WAIT = 200;
const SEARCH_LIMIT = 50;

/**
 * @typedef {import('./logic.js').Row} Row
 * @typedef {import('./logic.js').Target} Target
 * @typedef {{
 *   title: string, sub?: string|null, body: HTMLElement, kind?: string,
 *   back?: (() => void)|null,
 *   next?: {label: string|(() => string), ready: () => boolean, run: () => Promise<string|null>}|null,
 *   focus?: HTMLElement|null,
 * }} Screen 一屏：标题、下面一行、内容；`back` 是这一段自己的上一步（选一家那一屏没有，宿主用自己的）；`next` 是主按钮（`run`
 *   交回出错的字，成了是 `null`；`label` 是函数的跟着变，宿主每次 `ready` 时重读）
 * @typedef {{id: string, model: string|null, name: string}} Done 接好了：配置里的编号、选的模型、这一家的名字
 * @typedef {{show: (screen: Screen, dir: 1|-1) => void, ready: () => void, done: (result: Done) => void, panel: HTMLElement,
 *   manual?: () => void, mascot?: {busy: (on: boolean, hop?: boolean) => void, droop: () => void}|null}} Host
 *   宿主：画一屏（`dir` 是往前还是往回）、主按钮能不能点变了、接好了；下拉菜单开在哪一层上；照原来那张表手动填（设置页有）；
 *   吉祥物（有的话）
 */

/** 不换模型直接过：主对话现在用的那一个（`<编号>/<模型>`）。 @param {string} chat @returns {Done} */
const keptChat = (chat) => {
  const at = chat.indexOf('/');
  return { id: at > 0 ? chat.slice(0, at) : chat, model: at > 0 ? chat.slice(at + 1) : chat, name: chat };
};

export class ProviderFlow {
  /**
   * @param {any} ctx `settings` 包的 ctx（核心、字）
   * @param {Host} host
   * @param {'welcome'|'settings'} mode 引导的最后一步选模型、写 `models.chat`、建池，按钮写「下一步」；设置页只写这一家，按钮写「保存」
   */
  constructor(ctx, host, mode) {
    this.ctx = ctx;
    this.host = host;
    this.mode = mode;
    this.t = (/** @type {string} */ key, /** @type {any} */ fields) => ctx.text(key, fields);
    /** 读到的：常用的几家、本机的探测、配好了的编号、现在的主对话模型；读不到的原因 */
    this.featured = /** @type {any[]} */ ([]);
    this.detect = /** @type {any} */ (null);
    this.configured = /** @type {Set<string>} */ (new Set());
    this.chat = /** @type {string|null} */ (null);
    this.failed = /** @type {string|null} */ (null);
    /** 自定义的接口有哪几种（配置清单 `providers.<id>.driver` 的选项） */
    this.drivers = /** @type {{value: string, name: string}[]} */ ([]);
    /** 开着的那一屏的框改了：之前测的不算了 @type {(() => void)|null} */
    this.stale = null;
  }

  /** 读供应商的几样，画选一家那一屏。 */
  async start() {
    this.host.show(this.waiting(), 1);
    await this.load();
    this.host.show(this.list(), 1);
  }

  async load() {
    const core = this.ctx.core;
    const quiet = (/** @type {Promise<any>} */ p) => p.catch(() => null);
    try {
      const [featured, detect, models, schema] = await Promise.all([
        core.request('provider.catalog', { featured: true }), quiet(core.request('provider.detect', {})), quiet(core.request('model.list', {})), quiet(core.request('config.schema', {})),
      ]);
      this.featured = featured?.providers ?? [];
      this.detect = detect;
      this.configured = new Set((models?.providers ?? []).map((/** @type {any} */ p) => p.id));
      this.chat = models?.uses?.chat ?? null;
      const driver = (schema?.items ?? []).find((/** @type {any} */ i) => i.key === 'providers.<id>.driver');
      this.drivers = (driver?.options ?? []).map((/** @type {any} */ o) => ({ value: o.value, name: o.name ?? o.value }));
      this.failed = null;
    } catch (err) {
      this.failed = /** @type {any} */ (err)?.message ?? String(err);
    }
  }

  /** 读的时候那一屏。 */
  waiting() {
    return { title: this.t('onboard.title_list'), sub: this.t('onboard.sub_list'), body: h('p.ob-note', this.t('onboard.loading')), kind: 'wait' };
  }

  /** 选一家（第 5 条）。 */
  list() {
    const t = this.t;
    if (this.failed) return { title: t('onboard.title_list'), sub: t('onboard.sub_list'), body: h('p.ob-note.is-bad', t('onboard.load_failed', { reason: this.failed })) };
    const { common, local } = providerRows(this.featured, this.detect, this.configured);
    const groups = [h('div.ob-rows', common.map((r) => this.rowButton(r)))];
    if (local.length) groups.push(h('div.ob-group-name', t('onboard.local')), h('div.ob-rows', local.map((r) => this.rowButton(r))));
    groups.push(h('div.ob-group-name', t('onboard.other')), h('div.ob-rows',
      h('button.ob-row', { type: 'button', onclick: () => this.host.show(this.search(), 1) }, h('span.ob-row-name', t('onboard.search_more')), h('span.ob-row-arrow', icon('chevron-right'))),
      h('button.ob-row', { type: 'button', onclick: () => this.host.show(this.custom(), 1) }, h('span.ob-row-name', t('onboard.custom')), h('span.ob-row-hint', t('onboard.custom_hint')), h('span.ob-row-arrow', icon('chevron-right'))),
      this.host.manual ? h('button.ob-row', { type: 'button', onclick: () => this.host.manual?.() }, h('span.ob-row-name', t('onboard.manual')), h('span.ob-row-hint', t('onboard.manual_hint')), h('span.ob-row-arrow', icon('chevron-right'))) : null));
    const body = h('div.ob-list', groups);
    // 已经有主对话的模型（中途关了重来、在终端配过）：引导里能不换模型直接过
    const next = this.mode === 'welcome' && this.chat
      ? { label: t('onboard.next'), ready: () => true, run: async () => { this.host.done(keptChat(this.chat ?? '')); return null; } }
      : null;
    return { title: t('onboard.title_list'), sub: t('onboard.sub_list'), body, next, focus: /** @type {HTMLElement|null} */ (body.querySelector('.ob-row:not([disabled])')) };
  }

  /** 选一家的一行：名字、暗色的地址，右边标记、箭头；用不了的灰着。 @param {Row} r */
  rowButton(r) {
    const t = this.t;
    const tag = r.configured ? ['is-good', t('onboard.tag.configured')] : r.kind === 'catalog' && !r.supported ? ['is-off', t('onboard.tag.unsupported')] : r.kind === 'catalog' && r.env ? ['', t('onboard.tag.found')] : null;
    const off = r.kind === 'catalog' && !r.supported;
    return h(`button.ob-row${off ? '.is-off' : ''}`, { type: 'button', disabled: off, onclick: () => this.host.show(this.provider(r), 1) },
      'logo' in r ? logoEl(r.logo, r.name) : null, h('span.ob-row-name', r.name), r.kind === 'local' ? h('span.ob-row-hint', r.host) : null,
      tag ? h(`span.ob-tag${tag[0] ? `.${tag[0]}` : ''}`, tag[1]) : null, h('span.ob-row-arrow', icon('chevron-right')));
  }

  /** 搜索整份目录（第 12 条：常用里没有的）。 */
  search() {
    const t = this.t;
    const input = /** @type {HTMLInputElement} */ (h('input.set-input.ob-search', { type: 'text', placeholder: t('onboard.search_hint'), spellcheck: 'false', autocomplete: 'off' }));
    const out = h('div.ob-rows');
    let timer = 0;
    let asked = 0;
    const run = async () => {
      const n = ++asked;
      let got;
      try {
        got = await this.ctx.core.request('provider.catalog', { query: input.value.trim(), limit: SEARCH_LIMIT });
      } catch (err) {
        if (n === asked) replace(out, h('p.ob-note.is-bad', /** @type {any} */ (err)?.message ?? String(err)));
        return;
      }
      if (n !== asked) return;
      const { common } = providerRows(got?.providers ?? [], this.detect, this.configured);
      replace(out, common.length ? common.map((r) => this.rowButton(r)) : h('p.ob-note', t('onboard.no_match')));
    };
    input.addEventListener('input', () => {
      clearTimeout(timer);
      timer = window.setTimeout(run, SEARCH_WAIT);
    });
    run();
    return { title: t('onboard.search_title'), sub: null, body: h('div.ob-list', input, out), back: () => this.host.show(this.list(), -1), focus: input };
  }

  /**
   * 一家（第 6 条）：密钥、测试连接、选模型。
   * @param {Row} r
   */
  provider(r) {
    /** @type {Target} */
    const target = r.configured ? { kind: 'configured', id: r.configured } : r.kind === 'local' ? { kind: 'local', catalog: r.id } : { kind: 'catalog', catalog: r.id, env: r.env };
    const t = this.t;
    /** @type {HTMLInputElement|null} */
    let keyInput = null;
    let keyRow = null;
    if (target.kind === 'configured') keyRow = this.fieldRow(t('onboard.key'), h('span.ob-fixed', t('onboard.key_configured')));
    else if (target.kind === 'catalog') {
      keyInput = this.keyField(target.env ? t('onboard.key_env', { name: target.env }) : t('onboard.key_paste'));
      keyRow = this.fieldRow(t('onboard.key'), keyInput);
    }
    const key = () => {
      const text = keyInput?.value.trim() ?? '';
      if (text) return { value: text };
      return target.kind === 'catalog' && target.env ? { env: target.env } : null;
    };
    // 没找到变量、又没贴的测不了
    const canTest = () => target.kind !== 'catalog' || !!key();
    return this.testing(target, r.name, keyRow ? [keyRow] : [], key, canTest, keyInput, { auto: canTest() && !keyInput?.value, pasted: () => !!keyInput?.value.trim() });
  }

  /** 自定义（第 7 条）：地址、接口、密钥。 */
  custom() {
    const t = this.t;
    const url = /** @type {HTMLInputElement} */ (h('input.set-input.ob-input', { type: 'text', placeholder: 'https://…/v1', spellcheck: 'false', autocomplete: 'off' }));
    const drivers = this.drivers.length ? this.drivers : ['openai-chat', 'anthropic', 'openai-responses'].map((v) => ({ value: v, name: v }));
    let driver = drivers[0].value;
    // 接口：和设置页一样的下拉（三个名字放一排太挤）
    const seg = h('div.ob-select');
    const drawSeg = () => replace(seg, select({ panel: this.host.panel, ctx: this.ctx }, drivers, driver, (/** @type {string} */ v) => { driver = v; drawSeg(); this.stale?.(); }));
    drawSeg();
    const keyInput = this.keyField(t('onboard.key_optional'));
    const target = () => /** @type {Target} */ ({ kind: 'custom', driver, base_url: cleanUrl(url.value) });
    const rows = [this.fieldRow(t('onboard.url'), url), this.fieldRow(t('onboard.driver'), seg), this.fieldRow(t('onboard.key'), keyInput)];
    const key = () => (keyInput.value.trim() ? { value: keyInput.value.trim() } : null);
    url.addEventListener('input', () => this.stale?.());
    return this.testing(target, '', rows, key, () => validUrl(cleanUrl(url.value)), url, { auto: false, pasted: () => !!keyInput.value.trim(), title: t('onboard.custom_title') });
  }

  /** 密钥框：不回显，改了要重测。 @param {string} hint */
  keyField(hint) {
    const input = /** @type {HTMLInputElement} */ (h('input.set-input.ob-input', { type: 'password', placeholder: hint, spellcheck: 'false', autocomplete: 'off' }));
    input.addEventListener('input', () => this.stale?.());
    return input;
  }

  /** 一行：左边名字，右边控件。 @param {string} name @param {HTMLElement} control */
  fieldRow(name, control) {
    return h('div.ob-field', h('span.ob-field-name', name), h('div.ob-field-control', control));
  }

  /**
   * 测试连接、选模型那一段（一家、自定义共用）。主按钮只有一个（2026-10-10 项目主人：测试连接并进下一步）：没测过、改了框、没测成的写
   * 「测试连接」，测着写「正在测试…」，测成了写「下一步」（设置页「保存」）。
   * @param {Target|(() => Target)} targetOf 自定义的随框里写的变
   * @param {string} name 这一家的名字（标题；自定义的是空的，接好了照编号）
   * @param {HTMLElement[]} fields 上面的几行
   * @param {() => any} keyOf 这一次用的密钥
   * @param {() => boolean} canTest 测得了没有（贴了密钥、地址写对了）
   * @param {HTMLElement|null} first 先把焦点放哪
   * @param {{auto: boolean, pasted: () => boolean, title?: string}} opts 一进来就测；贴了密钥没有（存的时候要先存成密钥）
   * @returns {Screen}
   */
  testing(targetOf, name, fields, keyOf, canTest, first, opts) {
    const t = this.t;
    const target = () => (typeof targetOf === 'function' ? targetOf() : targetOf);
    const modelInput = /** @type {HTMLInputElement} */ (h('input.set-input.ob-input', { type: 'text', placeholder: t('onboard.model_name'), spellcheck: 'false', autocomplete: 'off' }));
    const modelRow = this.fieldRow(t('onboard.model_name'), modelInput);
    modelRow.hidden = true;
    const result = h('div.ob-result', { hidden: true });
    const models = h('div.ob-models', { hidden: true });
    /** 测的结果：成了的模型、选了哪个 */
    const state = { ok: /** @type {any} */ (null), chosen: /** @type {string|null} */ (null), running: false, gone: false };
    /** 测得了没有：没在测、贴了密钥（地址写对了）、要写模型名的写了 */
    const canRun = () => !state.running && canTest() && (modelRow.hidden || !!modelInput.value.trim());
    // 框里改了：之前测的不算了，主按钮回到「测试连接」
    this.stale = () => {
      if (state.ok) {
        state.ok = null;
        state.chosen = null;
        result.hidden = true;
        models.hidden = true;
      }
      this.host.ready();
    };
    modelInput.addEventListener('input', () => this.host.ready());
    const enter = (/** @type {KeyboardEvent} */ e) => {
      if (e.key !== 'Enter' || e.isComposing || state.ok) return;
      e.preventDefault();
      e.stopPropagation();
      if (canRun()) run();
    };
    for (const el of [...fields, modelRow]) el.addEventListener('keydown', enter);
    const run = async () => {
      if (!canTest()) return;
      state.running = true;
      state.ok = null;
      state.chosen = null;
      models.hidden = true;
      result.hidden = true;
      this.host.ready();
      this.host.mascot?.busy(true);
      let got;
      try {
        got = await this.ctx.core.request('provider.test', testParams(target(), keyOf(), modelRow.hidden ? null : modelInput.value.trim()));
      } catch (err) {
        got = { ok: false, stage: 'request', error: { class: 'other', message: /** @type {any} */ (err)?.message ?? String(err) } };
      }
      // 测的时候回到了选一家：结果不要了
      if (state.gone) return;
      state.running = false;
      result.hidden = false;
      if (got?.ok) {
        this.host.mascot?.busy(false);
        state.ok = got;
        const list = modelOrder(got.models ?? [], got.model ?? null);
        state.chosen = got.model ?? list[0] ?? null;
        result.className = 'ob-result is-good';
        replace(result, icon('check'), h('span', t('onboard.test_ok', { count: list.length })));
        this.drawModels(models, list, got.model ?? null, state);
        models.hidden = false;
      } else {
        this.host.mascot?.busy(false, false);
        this.host.mascot?.droop();
        const why = failureOf(got);
        result.className = 'ob-result is-bad';
        replace(result, h('div.ob-result-line', icon('x'), h('span', t(why.key) + (why.status ? ` · ${why.status}` : ''))), why.message ? h('p.ob-result-raw', why.message) : null);
        if (why.needsModel) {
          modelRow.hidden = false;
          queueMicrotask(() => modelInput.focus());
        }
      }
      this.host.ready();
    };
    const body = h('div.ob-provider', fields, modelRow, result, models);
    if (opts.auto) queueMicrotask(run);
    const save = async () => {
      if (!state.ok) return null;
      return this.save(target(), keyOf(), opts.pasted(), state.chosen, name);
    };
    return {
      title: opts.title ?? name,
      sub: null,
      body,
      back: () => {
        state.gone = true;
        this.stale = null;
        this.host.mascot?.busy(false, false);
        this.host.show(this.list(), -1);
      },
      next: {
        label: () => (state.ok ? t(this.mode === 'welcome' ? 'onboard.next' : 'onboard.save') : t(state.running ? 'onboard.testing' : 'onboard.test')),
        ready: () => (state.ok ? this.mode === 'settings' || !!state.chosen : canRun()),
        run: async () => {
          if (state.ok) return save();
          await run();
          return null;
        },
      },
      focus: first,
    };
  }

  /**
   * 一列模型：搜索框、单选，试的那一个后面写「推荐」。
   * @param {HTMLElement} box @param {string[]} list @param {string|null} tried @param {{chosen: string|null}} state
   */
  drawModels(box, list, tried, state) {
    const t = this.t;
    if (this.mode !== 'welcome') return replace(box);
    const find = /** @type {HTMLInputElement} */ (h('input.set-input.ob-search', { type: 'text', placeholder: t('onboard.model_search'), spellcheck: 'false', autocomplete: 'off' }));
    const rows = h('div.ob-model-list');
    const draw = () => {
      const shown = filterNames(list, find.value);
      replace(rows, shown.length ? shown.slice(0, MAX_MODELS).map((m) => h(`button.ob-model${m === state.chosen ? '.is-on' : ''}`, { type: 'button', onclick: () => { state.chosen = m; draw(); this.host.ready(); } },
        h('span.ob-model-name', m), m === tried ? h('span.ob-model-note', t('onboard.recommended')) : null, m === state.chosen ? h('span.ob-model-check', icon('check')) : null)) : h('p.ob-note', t('onboard.no_match')),
      shown.length > MAX_MODELS ? h('p.ob-note', t('onboard.more_models', { shown: MAX_MODELS })) : null);
    };
    find.addEventListener('input', draw);
    draw();
    replace(box, list.length > 8 ? find : null, rows);
  }

  /**
   * 存（第 6 条）：贴了密钥的先存成新名字的密钥，再一条 `config.set` 写进个人设置（同设置页）；交回出错的字，成了叫宿主 `done`。
   * @param {Target} target @param {any} key 这一次用的密钥 @param {boolean} pasted @param {string|null} model @param {string} name
   */
  async save(target, key, pasted, model, name) {
    const core = this.ctx.core;
    try {
      const welcome = this.mode === 'welcome';
      const items = welcome ? (await core.request('config.get', {}))?.items ?? {} : {};
      const taken = this.configured;
      const id = setupChanges(target, null, { taken }).id;
      /** @type {{env: string}|{secret: string}|null} */
      let ref = key?.env ? { env: key.env } : null;
      if (pasted && key?.value) {
        const secret = secretName(id, Date.now());
        await core.request('secret.set', { name: secret, value: key.value });
        ref = { secret };
      }
      const { changes } = setupChanges(target, ref, { chat: welcome ? model : null, pools: welcome && noPools(items), taken });
      // 写个人设置，同设置页（2026-10-10 项目主人：引导里选了 DeepSeek，会话照旧是原来的 magpie——原来写系统配置，被个人设置里写着的盖住了）
      if (changes.length) await core.request('config.set', { layer: 'personal', changes });
      this.configured.add(id);
      this.stale = null;
      this.host.done({ id, model, name: name || id });
      return null;
    } catch (err) {
      const e = /** @type {any} */ (err);
      return this.t('onboard.save_failed', { reason: e?.data?.message ?? e?.message ?? String(err) });
    }
  }
}

