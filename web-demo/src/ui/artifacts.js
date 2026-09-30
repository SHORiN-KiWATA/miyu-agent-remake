// @ts-check
//! 预览工作区（蓝图 `web.md`「预览工作区」，照旧版 `index.html:314-362`、`app.js:5475-6448`、`styles.css:4893-5476`）：
//! 右边一栏，打开时顶替会话概况；放这个会话的产物（`model/artifacts.js`），一次看一个。
//!
//! - 头上一行：预览、源码的切换（两样都有才露）；标题按钮（名字、类型，点开是全部产物的列表）；复制、下载、最大化、关；
//! - 按类型画：Markdown 照她的回答那一套；代码上色、带行号；HTML 在沙盒 iframe 里直接跑（`srcdoc`，只给脚本和弹窗，
//!   内容安全策略不许联网，照旧版）；图片滚轮缩放、拖动；视频原生控制条；别的当文字；
//! - 文件的字经桥的 `/file` 取（以后是核心的网页模块）；同一个文件改过（序号变了）才重取；
//! - 宽度能拖，记在这个浏览器里；来了新的、工作区关着时，开关按钮上一个圆点；窗口够宽时自动打开。

import { h, icon, replace } from './dom.js';
import { show, hide } from '../lib/motion.js';
import { res, t } from '../util/res.js';
import { artifacts, artifactLanguage } from '../model/artifacts.js';
import { fileUrl } from '../core/host.js';
import { renderMarkdown } from '../markdown/render.js';
import { paint } from '../markdown/highlight.js';
import { copy } from '../markdown/build.js';
import { richHooks } from './rich.js';

/** 拖到多宽，记在这台设备上的名字（内核的 `storage` 会带上账号；这个终端的状态） */
const WIDTH = 'artifacts_width';

export class Artifacts {
  /**
   * @param {import('../core/connection.js').Connection} conn 问桥约定目录的真实位置（`web.realpath`）
   * @param {HTMLElement} shell 整页（`.app-shell`）：开关时换它的类、宽度
   * @param {(text: string, good?: boolean) => void} say 提示（复制了几个字）
   * @param {() => void} changed 开、关以后整页重画（会话概况要让位）
   * @param {import('./rich.js').Ext} ext 软件包接进来的：Markdown 预览里的代码块、图片照它（mermaid、灯箱这类）
   */
  constructor(conn, shell, say, changed, ext) {
    this.ext = ext;
    this.conn = conn;
    this.shell = shell;
    this.say = say;
    this.changed = changed;
    this.open = false;
    this.maximized = false;
    this.mode = 'preview';
    /** @type {import('../model/artifacts.js').Artifact[]} */
    this.list = [];
    this.current = /** @type {string|null} */ (null);
    /** 每个会话约定目录的真实位置：`会话` → 位置（问过还没回的是 `undefined`）。 */
    this.dirs = new Map();
    /** 每个会话看过的最大序号：比它新的算「来了新的」。 */
    this.seen = new Map();
    this.where = /** @type {import('./rich.js').Where} */ ({ session: null, home: null, cwd: null });
    this.drawn = '';
    /** 当前这一个的原文（复制源码用）；还没取到的是 `null`。 */
    this.text = /** @type {string|null} */ (null);
    this.token = 0;
    this.toggle = h('button.icon-button.artifact-toggle', { type: 'button', hidden: true, title: t('artifacts.toggle'), onclick: () => this.setOpen(!this.open) }, icon('panel-right'));
    this.previewButton = h('button.artifact-mode', { type: 'button', title: t('artifacts.preview'), onclick: () => this.setMode('preview') }, icon('eye'));
    this.sourceButton = h('button.artifact-mode', { type: 'button', title: t('artifacts.source'), onclick: () => this.setMode('source') }, icon('code-2'));
    this.modes = h('div.artifact-mode-switch', this.previewButton, this.sourceButton);
    this.title = h('strong');
    this.badge = h('small');
    this.menu = h('div.artifact-resource-menu', { role: 'menu', hidden: true });
    this.titleButton = h('button.artifact-title-button', { type: 'button', title: t('artifacts.list'), onclick: () => (this.menu.hidden || this.menu.classList.contains('is-leaving') ? show(this.menu) : hide(this.menu)) },
      this.title, this.badge, icon('chevron-down'));
    this.download = h('a.icon-button.artifact-action', { title: t('artifacts.download'), 'aria-label': t('artifacts.download'), download: '' }, icon('download'));
    this.maxButton = h('button.icon-button.artifact-action', { type: 'button', onclick: () => this.setMaximized(!this.maximized) });
    this.view = h('div.artifact-view');
    this.el = h('aside.artifact-workspace', { hidden: true },
      h('div.artifact-resize-handle', { title: t('artifacts.resize'), onpointerdown: (e) => this.resize(e) }),
      h('header.artifact-header',
        h('div.artifact-leading', this.modes, h('div.artifact-resource-wrap', this.titleButton, this.menu)),
        h('div.artifact-actions',
          h('button.icon-button.artifact-action', { type: 'button', title: t('artifacts.copy'), onclick: () => this.copySource() }, icon('copy')),
          this.download, this.maxButton,
          h('button.icon-button.artifact-action', { type: 'button', title: t('artifacts.close'), onclick: () => this.setOpen(false) }, icon('x')))),
      this.view);
    document.addEventListener('click', (e) => {
      if (!this.menu.hidden && !/** @type {Element} */ (e.target).closest?.('.artifact-resource-wrap')) hide(this.menu);
    });
    this.setWidth(Number(ext.storage?.get(WIDTH, 0)) || res.artifacts.width);
    this.setMaximized(false);
  }

  /**
   * 照这个会话的事件更新：列表、开关按钮、自动打开。
   * @param {any[]} events
   * @param {import('./rich.js').Where} where
   */
  update(events, where) {
    this.where = where;
    const session = where.session;
    const dir = session ? this.dirOf(session, where.cwd) : null;
    this.list = artifacts(events, dir ?? null);
    const newest = this.list[0]?.seq ?? 0;
    const seen = session ? this.seen.get(session) : undefined;
    // 打开会话时已经有的不算新来的：约定目录的位置问到了（不是 undefined）才记下起点
    if (session && dir !== undefined && seen === undefined) this.seen.set(session, newest);
    const fresh = session != null && seen !== undefined && newest > seen;
    if (fresh && session) {
      this.seen.set(session, newest);
      this.current = this.list[0].path;
      // 窗口够宽的自动打开；窄的只在开关按钮上点一个圆点
      if (innerWidth > res.artifacts.auto_open_from) this.setOpen(true);
      else this.toggle.classList.add('has-new');
    }
    if (!this.list.some((a) => a.path === this.current)) this.current = this.list[0]?.path ?? null;
    this.toggle.hidden = !this.list.length;
    if (!this.list.length && this.open) this.setOpen(false);
    if (this.open) this.draw();
  }

  /** 约定目录的真实位置：问过的直接给；没问过的问桥（回来以后整页重画一次），还没回的是 `undefined`。 */
  dirOf(session, cwd) {
    if (!cwd) return null;
    const key = `${session}\n${cwd}`;
    if (!this.dirs.has(key)) {
      this.dirs.set(key, undefined);
      this.conn.request('web.realpath', { path: `${cwd}/${res.artifacts.dir}` })
        .then((r) => { this.dirs.set(key, r?.path ?? null); this.changed(); })
        .catch((err) => console.error(`问不到产物目录的真实位置：${err.message}`));
    }
    return this.dirs.get(key);
  }

  /** 开、关。 */
  setOpen(open) {
    this.open = open && this.list.length > 0;
    // 那一列长出来、收回去，里面跟着淡入淡出（蓝图「动效」）
    if (this.open) show(this.el);
    else hide(this.el);
    this.shell.classList.toggle('has-artifacts', this.open);
    this.toggle.classList.toggle('is-active', this.open);
    if (this.open) this.toggle.classList.remove('has-new');
    if (!this.open && this.maximized) this.setMaximized(false);
    if (this.open) this.draw(true);
    this.changed();
  }

  setMaximized(on) {
    this.maximized = on;
    this.el.classList.toggle('is-maximized', on);
    replace(this.maxButton, icon(on ? 'minimize-2' : 'maximize-2'));
    this.maxButton.title = t(on ? 'artifacts.restore' : 'artifacts.maximize');
  }

  setMode(mode) {
    this.mode = mode;
    this.draw(true);
  }

  /** 宽度：窗口宽的几成，夹在 `width_min`、`width_max` 之间。 */
  setWidth(ratio) {
    const a = res.artifacts;
    this.ratio = Math.min(a.width_max, Math.max(a.width_min, ratio));
    this.shell.style.setProperty('--artifact-ratio', String(this.ratio));
  }

  /** 拖左边那条改宽度，松手记下来。 */
  resize(/** @type {PointerEvent} */ e) {
    const handle = /** @type {HTMLElement} */ (e.currentTarget);
    handle.setPointerCapture(e.pointerId);
    const move = (/** @type {PointerEvent} */ m) => this.setWidth((innerWidth - m.clientX) / innerWidth);
    const up = () => {
      handle.removeEventListener('pointermove', move);
      this.ext.storage?.set(WIDTH, this.ratio);
    };
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', up, { once: true });
  }

  /** 画当前这一个；`force` 为假时，同一个文件没改过就不重画。 */
  draw(force = false) {
    const a = this.list.find((x) => x.path === this.current);
    const session = this.where.session;
    if (!a || !session) return;
    const sig = `${a.path}\n${a.seq}\n${this.mode}`;
    if (!force && sig === this.drawn) return;
    this.drawn = sig;
    const hasPreview = a.kind !== 'code' && a.kind !== 'text';
    const hasSource = a.kind !== 'image' && a.kind !== 'video';
    const mode = hasPreview && hasSource ? this.mode : hasPreview ? 'preview' : 'source';
    this.modes.hidden = !(hasPreview && hasSource);
    this.previewButton.classList.toggle('is-active', mode === 'preview');
    this.sourceButton.classList.toggle('is-active', mode === 'source');
    this.title.textContent = a.name;
    this.badge.textContent = t(`artifacts.types.${a.kind}`);
    const url = `${fileUrl(session, a.path)}&v=${a.seq}`;
    this.download.setAttribute('href', fileUrl(session, a.path, true));
    this.drawMenu();
    const token = ++this.token;
    if (a.kind === 'image' && mode === 'preview') return replace(this.view, imageStage(url, a.name));
    if (a.kind === 'video') return replace(this.view, h('div.artifact-video', h('video', { src: url, controls: true, preload: 'metadata', playsinline: true })));
    replace(this.view, h('div.artifact-loading', icon('loader-circle'), t('artifacts.loading')));
    fetch(url, { cache: 'no-store' })
      .then((r) => (r.ok ? r.text() : Promise.reject(new Error(`${r.status} ${r.statusText}`))))
      .then((text) => {
        if (token !== this.token) return;
        this.text = text;
        replace(this.view, mode === 'source' ? source(text, a.path) : preview(a.kind, text, this.where, this.say, this.ext));
      })
      .catch((err) => {
        if (token !== this.token) return;
        replace(this.view, h('div.artifact-failure', icon('circle-alert'), t('artifacts.failed_with', { reason: err.message })));
      });
  }

  /** 全部产物的列表：当前的打勾，点哪个看哪个。 */
  drawMenu() {
    replace(this.menu, this.list.map((a) => h(`button.artifact-resource-item${a.path === this.current ? '.is-current' : ''}`, {
      type: 'button', role: 'menuitem',
      onclick: () => { this.current = a.path; hide(this.menu); this.draw(true); },
    }, h('span.artifact-resource-name', a.name), h('small', t(`artifacts.types.${a.kind}`)), a.path === this.current ? icon('check') : h('span'))));
  }

  /** 复制源码（图片、视频复制路径）。 */
  copySource() {
    const a = this.list.find((x) => x.path === this.current);
    if (!a) return;
    copy(a.kind === 'image' || a.kind === 'video' ? a.path : this.text ?? '', this.say);
  }
}

/** 预览：Markdown 照她的回答那一套；HTML 在沙盒里跑。 */
function preview(kind, text, where, say, ext) {
  if (kind === 'markdown') {
    const body = h('article.markdown-body.artifact-markdown');
    renderMarkdown(body, text, { hooks: richHooks(where, say, ext)('artifact'), say });
    return body;
  }
  // HTML：`srcdoc` 放进沙盒（没有 allow-same-origin，碰不到页面），最前面垫一条不许联网的内容安全策略
  // 内容安全策略照 `artifacts.json` 的 `html_csp`（照旧版 `assets.rs:965-989`，去掉取本站的那几样）
  const doc = `<meta http-equiv="Content-Security-Policy" content="${res.artifacts.html_csp}">${text}`;
  return h('iframe.artifact-frame', { sandbox: 'allow-scripts allow-modals', srcdoc: doc, title: 'artifact' });
}

/** 源码：左边行号，右边上色的代码（照旧版 `app.js:6204-6230`）。 */
function source(text, path) {
  const lines = text.split('\n');
  const code = h('code', text);
  const lang = artifactLanguage(path);
  if (lang) paint(code, lang, text, true);
  return h('div.artifact-source',
    h('div.artifact-line-numbers', { 'aria-hidden': 'true' }, lines.map((_, i) => h('span', String(i + 1)))),
    h('pre.artifact-code', code));
}

/** 图片：滚轮缩放（倍数照 `artifacts.json` 的 `zoom`），放大了能拖（照旧版 `app.js:6017-6063`）。 */
function imageStage(url, name) {
  const img = h('img', { src: url, alt: name, draggable: 'false' });
  const stage = h('div.artifact-image-stage', img);
  let zoom = 1;
  let x = 0;
  let y = 0;
  const apply = () => { img.style.transform = `translate(${x}px, ${y}px) scale(${zoom})`; };
  stage.addEventListener('wheel', (e) => {
    e.preventDefault();
    const z = res.artifacts.zoom;
    zoom = Math.min(z.max, Math.max(z.min, zoom * (e.deltaY < 0 ? z.in : z.out)));
    if (zoom <= 1) { x = 0; y = 0; }
    apply();
  }, { passive: false });
  stage.addEventListener('pointerdown', (e) => {
    if (zoom <= 1) return;
    const start = { px: e.clientX, py: e.clientY, x, y };
    stage.setPointerCapture(e.pointerId);
    stage.classList.add('is-dragging');
    const move = (/** @type {PointerEvent} */ m) => { x = start.x + m.clientX - start.px; y = start.y + m.clientY - start.py; apply(); };
    stage.addEventListener('pointermove', move);
    stage.addEventListener('pointerup', () => { stage.removeEventListener('pointermove', move); stage.classList.remove('is-dragging'); }, { once: true });
  });
  return stage;
}
