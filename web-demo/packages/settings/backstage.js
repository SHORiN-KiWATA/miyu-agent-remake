// @ts-check
//! 「软件后台」页（蓝图 `web.md`「设置页」第 19 条；核心 F-6 中，`package-pages.md`；2026-10-10 项目主人：软件后台只列带自己页面的软件，
//! 照 AstrBot 的插件那样由软件自己带页面）：一个一行，点进去滑进它自己的页面。页面放在隔离的框里（`sandbox="allow-scripts allow-forms"`，
//! 不给 `allow-same-origin`：来源是空的，读不到网页的存储、口令；桥的响应头不许它联网）。框的地址经宿主请桥造票据（`host.pages.open`）。
//!
//! 框和网页之间（`FrameHost`）：框第一次载入完，网页造一个 `MessageChannel`，把一头交给框（只交这一次，后台页做成单页）；以后照 JSON-RPC
//! 的写法说话，网页认的一律照它给框的那个包，框说自己是谁不算数。方法只有 `context`、`settings.get`、`settings.set`（键要是它自己的，
//! 密钥类型给一段字的先存成密钥再写引用）、`call`（转 `package.call`）；推送 `settings.changed`、`theme.changed`。

import { h, icon, hasIcon } from '../../src/lib/dom.js';
import { secretName } from './model.js';

/** 带给框的几个颜色：网页的样式变量 → 交出去的名字 */
const COLORS = /** @type {const} */ ([['--accent', 'accent'], ['--surface', 'surface'], ['--surface-2', 'surface_2'], ['--text', 'text'], ['--text-soft', 'text_soft'], ['--line', 'line'], ['--danger', 'danger']]);

/** 「软件后台」页：带后台页的软件一个一行；一个都没有的写一句。 @param {any} dialog */
export function drawBackstage(dialog) {
  const t = (/** @type {string} */ key) => dialog.ctx.text(key);
  const list = (dialog.packages ?? []).filter((p) => p.page);
  if (!list.length) return [h('p.set-empty', t('backstage.none'))];
  return [h('section.set-group', h('div.set-rows', list.map((p) => {
    const state = p.status === 'running' || p.status === 'stopped' || p.status === 'program_missing' ? h(`span.set-pkg-status${p.status === 'running' ? '.is-on' : p.status === 'program_missing' ? '.is-bad' : ''}`, t(`pkg.status.${p.status}`)) : null;
    return h('div.set-row.set-pkg.is-expandable', { onclick: () => openBackstage(dialog, p.package) },
      h('span.set-pkg-face', icon(p.icon && hasIcon(p.icon) ? p.icon : 'package')),
      h('div.set-text', h('div.set-name', h('span', p.name ?? p.package)), p.summary ? h('p.set-desc', p.summary) : null),
      state, h('span.set-pkg-arrow', icon('chevron-right')));
  })))];
}

/** 打开一个软件的后台页：先到「软件后台」页，再滑进它的页面（软件包信息页上的「打开软件后台」、`miyu web --package` 都走这里）。 @param {any} dialog @param {string} id */
export function openBackstage(dialog, id) {
  if (dialog.current !== 'backstage') dialog.show('backstage');
  const p = (dialog.packages ?? []).find((x) => x.package === id);
  let node = /** @type {HTMLElement|null} */ (null);
  dialog.openSub(p?.name ?? id, () => (node ??= framePage(dialog, id)), { keep: true, leave: () => dialog.frame?.close() });
}

/** 后台页那一层：框、下面一行写是谁提供的。框的地址读到了再填；读不到的写一句。 @param {any} dialog @param {string} id */
function framePage(dialog, id) {
  const t = (/** @type {string} */ key, /** @type {any} */ fields) => dialog.ctx.text(key, fields);
  const p = (dialog.packages ?? []).find((x) => x.package === id);
  const frame = /** @type {HTMLIFrameElement} */ (h('iframe.set-backstage-frame', { sandbox: 'allow-scripts allow-forms', referrerpolicy: 'no-referrer', title: p?.name ?? id }));
  const box = h('div.set-backstage', frame, h('p.set-backstage-by', t('backstage.by', { name: p?.name ?? id })));
  dialog.frame?.close();
  dialog.frame = new FrameHost(dialog, id, frame);
  Promise.resolve(dialog.ctx.host.pages?.open(id)).then((url) => {
    if (url) frame.src = url;
    else frame.replaceWith(h('p.set-empty.is-bad', t('backstage.failed')));
  });
  return box;
}

/** 框和网页之间的那条通道。 */
export class FrameHost {
  /** @param {any} dialog @param {string} id 框是哪个包的 @param {HTMLIFrameElement} frame */
  constructor(dialog, id, frame) {
    this.dialog = dialog;
    this.id = id;
    this.frame = frame;
    /** @type {MessagePort|null} */
    this.port = null;
    this.handed = false;
    this.lookSig = '';
    // 只在第一次载入完交通道：框后来再载入（跳到别的地址）不再交，网页分不出载入的是谁
    frame.addEventListener('load', () => {
      if (this.handed || !frame.src) return;
      this.handed = true;
      const channel = new MessageChannel();
      this.port = channel.port1;
      this.port.onmessage = (e) => { this.handle(e.data); };
      frame.contentWindow?.postMessage({ miyu: 'port' }, '*', [channel.port2]);
    });
    // 网页换了主题、系统换了深浅：推 `theme.changed`
    this.watch = new MutationObserver(() => this.themeMaybeChanged());
    this.watch.observe(document.documentElement, { attributes: true });
    this.watch.observe(document.body, { attributes: true });
    this.media = matchMedia('(prefers-color-scheme: dark)');
    this.onMedia = () => this.themeMaybeChanged();
    this.media.addEventListener('change', this.onMedia);
    this.lookSig = JSON.stringify(this.look());
  }

  /** 收掉：通道关了、不再看主题。 */
  close() {
    this.port?.close();
    this.port = null;
    this.watch.disconnect();
    this.media.removeEventListener('change', this.onMedia);
    if (this.dialog.frame === this) this.dialog.frame = null;
  }

  /** 网页这时的深浅和几个颜色（照算出来的颜色，`rgb(…)`）。 */
  look() {
    const style = getComputedStyle(document.body);
    const probe = document.createElement('i');
    document.body.append(probe);
    const colors = Object.fromEntries(COLORS.map(([v, name]) => {
      probe.style.color = style.getPropertyValue(v).trim() || 'inherit';
      return [name, getComputedStyle(probe).color];
    }));
    probe.remove();
    const rgb = (colors.surface.match(/[\d.]+/g) ?? ['255', '255', '255']).slice(0, 3).map(Number);
    const light = (0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]) / 255 > 0.5;
    return { theme: light ? 'light' : 'dark', colors };
  }

  themeMaybeChanged() {
    const look = this.look();
    const sig = JSON.stringify(look);
    if (sig === this.lookSig) return;
    this.lookSig = sig;
    this.push('theme.changed', look);
  }

  /** 配置变了（`config.changed`，网页重读完以后）：有它的项的推 `settings.changed`。 @param {string[]} keys */
  configChanged(keys) {
    const mine = keys.filter((k) => k.startsWith(`${this.id}.`));
    if (mine.length) this.push('settings.changed', { keys: mine });
  }

  /** @param {string} method @param {any} params */
  push(method, params) {
    this.port?.postMessage({ jsonrpc: '2.0', method, params });
  }

  /** @param {any} id @param {any} result @param {any} [error] */
  reply(id, result, error) {
    if (id === undefined || id === null || !this.port) return;
    this.port.postMessage(error ? { jsonrpc: '2.0', id, error } : { jsonrpc: '2.0', id, result });
  }

  /** 框发来的一条。 @param {any} msg */
  async handle(msg) {
    if (!msg || typeof msg !== 'object' || msg.jsonrpc !== '2.0' || typeof msg.method !== 'string') {
      this.reply(msg?.id ?? null, null, { code: -32600, message: 'Invalid Request' });
      return;
    }
    const p = msg.params ?? {};
    const core = this.dialog.ctx.core;
    try {
      if (msg.method === 'context') this.reply(msg.id, { package: this.id, language: document.documentElement.lang, ...this.look() });
      else if (msg.method === 'settings.get') this.reply(msg.id, this.settings());
      else if (msg.method === 'settings.set') this.reply(msg.id, await this.write(p.changes));
      else if (msg.method === 'call') this.reply(msg.id, await core.request('package.call', { package: this.id, method: p.method, ...(p.params === undefined ? {} : { params: p.params }) }));
      else this.reply(msg.id, null, { code: -32601, message: 'Method not found' });
    } catch (err) {
      const e = /** @type {any} */ (err);
      this.reply(msg.id, null, { code: e?.code ?? -32000, message: e?.message ?? String(err), ...(e?.data ? { data: e.data } : e?.reason ? { data: { reason: e.reason } } : {}) });
    }
  }

  /** 它自己的设置项：配置清单里 `package` 是它的那几项，和最终值（密钥照 `config.get` 的写法，是引用不是值）。 */
  settings() {
    const items = (this.dialog.schema?.items ?? []).filter((/** @type {any} */ i) => i.package === this.id);
    const values = Object.fromEntries(items.map((/** @type {any} */ i) => [i.key, this.dialog.got?.items?.[i.key]?.value ?? i.default ?? null]));
    return { items, values };
  }

  /**
   * 写它自己的设置项（照 `config.set` 写进系统配置）：每一项的键都要是 `<它的编号>.` 开头，不是的整个不办（`forbidden`）；密钥类型给的是
   * 一段字的，先 `secret.set` 存成一个新名字的密钥，再写引用（同设置页的密钥框；核心 d77e4263）。
   * @param {any} changes
   */
  async write(changes) {
    if (!Array.isArray(changes) || !changes.length || changes.some((c) => typeof c?.key !== 'string')) throw Object.assign(new Error('Invalid params'), { code: -32602 });
    if (changes.some((c) => !c.key.startsWith(`${this.id}.`))) throw Object.assign(new Error(this.dialog.ctx.text('backstage.forbidden')), { code: -32010, data: { reason: 'forbidden' } });
    const types = new Map((this.dialog.schema?.items ?? []).filter((/** @type {any} */ i) => i.package === this.id).map((/** @type {any} */ i) => [i.key, i.type]));
    const core = this.dialog.ctx.core;
    const out = [];
    for (const c of changes) {
      if (types.get(c.key) === 'secret' && typeof c.value === 'string') {
        const name = secretName(c.key.toLowerCase().replace(/[^a-z0-9]+/g, '-'), Date.now());
        await core.request('secret.set', { name, value: c.value });
        out.push({ key: c.key, value: { secret: name } });
      } else out.push(c);
    }
    return core.request('config.set', { layer: 'system', changes: out });
  }
}
