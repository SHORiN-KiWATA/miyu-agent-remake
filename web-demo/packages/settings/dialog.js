// @ts-check
//! 设置页的弹窗（蓝图 `web.md`「设置页」第 1–4、6、9、10、15 条）：框、左边的分页、搜索、右边一页页；核心的配置改一项存一项
//! （`config.set`），别处改了（`config.changed`）重读重画，正在改的那一项不被冲掉。各页怎么画在 `rows.js`、`look.js`、`models.js`。

import { h, icon, replace } from '../../src/lib/dom.js';
import { leave } from '../../src/lib/motion.js';
import { buildPages, search, pagesWithErrors, layerFor, expectFor } from './model.js';
import { coreRow, groupBlock, banner } from './rows.js';
import { drawLook } from './look.js';
import { drawPackages } from './packages.js';
import { drawModels } from './models.js';
import { sectionKit } from './kit.js';

/** 上次看的那一页：这个终端记着（蓝图第 3 条），刷新就忘 */
let lastPage = 'general';

export class SettingsDialog {
  /** @param {any} ctx */
  constructor(ctx) {
    this.ctx = ctx;
    this.isOpen = false;
    /** @type {import('./model.js').Page[]} */
    this.pages = [];
    this.schema = null;
    this.got = null;
    this.models = null;
    this.current = lastPage;
    this.query = '';
    /** 别处改了、正在改的那一项有焦点：等焦点走了再重画 */
    this.pending = false;
    this.onKey = this.onKey.bind(this);
    this.onPush = this.onPush.bind(this);
  }

  /** 打开：画框，读配置，到 `page` 那一页（没给的回上次看的）。 */
  async open(page) {
    this.isOpen = true;
    const cfg = this.ctx.config;
    this.search = h('input.set-search', { type: 'search', placeholder: this.ctx.text('search'), 'aria-label': this.ctx.text('search'), oninput: () => { this.query = this.search.value; this.drawBody(); } });
    this.nav = h('nav.set-nav');
    this.title = h('strong.set-title');
    this.body = h('div.set-body');
    this.panel = h('section.set-panel', { role: 'dialog', 'aria-modal': 'true', 'aria-label': this.ctx.text('title'), style: `--set-w: ${cfg.width}px; --set-h: ${cfg.height}px` },
      h('aside.set-side', h('label.set-search-box', icon('search'), this.search), this.nav),
      h('div.set-main',
        h('header.set-head', h('button.set-back', { type: 'button', onclick: () => this.root.classList.remove('is-inside') }, icon('chevron-left'), this.ctx.text('back')), this.title,
          h('button.icon-button.set-close', { type: 'button', title: this.ctx.text('close'), 'aria-label': this.ctx.text('close'), onclick: () => this.close() }, icon('x'))),
        this.body));
    this.root = h('div.set-layer', h('div.set-scrim', { onclick: () => this.close() }), this.panel);
    this.body.addEventListener('focusout', () => setTimeout(() => this.flush(), 0));
    document.body.append(this.root);
    this.narrow = matchMedia(`(max-width: ${cfg.narrow}px)`);
    document.addEventListener('keydown', this.onKey, true);
    this.ctx.core.onPush(this.onPush);
    this.current = page ?? lastPage;
    this.body.replaceChildren(h('p.set-empty', this.ctx.text('loading')));
    this.search.focus();
    try {
      await this.load();
    } catch (err) {
      this.body.replaceChildren(h('p.set-empty.is-bad', this.ctx.text('load_failed', { reason: err.message })));
      return;
    }
    this.show(this.current);
    // 窄的时候（第 15 条）：没点名哪一页的先列分页
    if (!page && this.narrow.matches) this.root.classList.remove('is-inside');
  }

  /** 关：淡出、拿掉；焦点回到原来的地方。 */
  close() {
    if (!this.isOpen) return;
    this.isOpen = false;
    document.removeEventListener('keydown', this.onKey, true);
    this.ctx.core.pushes?.delete(this.onPush);
    leave(this.root, () => this.root.remove());
  }

  /** 读核心的配置清单、最终值；模型列表另读（可能慢：核心要去供应商那边拉），读完了模型页跟着重画。 */
  async load() {
    const core = this.ctx.core;
    const [schema, got, packages] = await Promise.all([core.request('config.schema', {}), core.request('config.get', { all: true }),
      // 核心的软件包（9-1）：「默认界面」的选项照它列（读不到的旧核心是空的）
      core.request('package.list', {}).then((r) => r?.packages ?? [], () => [])]);
    /** 能直接敲 `miyu` 打开的界面（`kind` 是 `ui`、有命令的包，核心 9-3） */
    this.heads = packages.filter((p) => p.kind === 'ui' && p.command);
    this.schema = schema;
    this.got = got;
    const cfg = this.ctx.config;
    this.pages = buildPages(schema, got, { merge: cfg.merge, moveGroups: cfg.move_groups, hide: cfg.hide_prefixes });
    this.modelsPromise = this.loadModels();
  }

  /** 读模型列表：读着的时候 `modelsLoading`，读完了在模型页的重画一次（正在改一项的不打断）。 */
  async loadModels() {
    this.modelsLoading = true;
    try {
      this.models = await this.ctx.core.request('model.list', {});
    } catch {
      // 读不到的留着上一次的；第一次就读不到的，模型页写读不到
    } finally {
      this.modelsLoading = false;
      if (this.isOpen && this.current === 'models' && !this.query && !this.editing()) this.drawBody();
    }
  }

  /** 左栏的分页：核心的照 `pages`，网页的「外观」「软件包」，挂进 `settings.section` 的；先后照设置项 `order`。 */
  entries() {
    const order = this.ctx.config.order;
    const named = (id) => this.ctx.text(`pages.${id}`);
    const all = [
      ...this.pages.map((p) => ({ id: p.id, name: p.name })),
      { id: 'appearance', name: named('appearance') },
      // 核心也有一页「软件包」（9-1 下，一个包一组）：和网页自己的组件合成一页，上面核心、下面网页（2026-10-07 项目主人定）
      ...(this.pages.some((p) => p.id === 'packages') ? [] : [{ id: 'packages', name: named('packages') }]),
      ...this.ctx.slots.list('settings.section').map((s) => ({ id: s.id, name: s.name, section: s })),
    ];
    const rank = (id) => {
      const i = order.indexOf(id);
      return i >= 0 ? i : order.indexOf('advanced') - 0.5;
    };
    return all.sort((a, b) => rank(a.id) - rank(b.id));
  }

  /** 到某一页：左栏选中它，右边画它；窄的时候点进去。 */
  show(id) {
    if (!this.entries().some((e) => e.id === id)) id = 'general';
    this.current = id;
    lastPage = id;
    this.query = '';
    this.search.value = '';
    this.root.classList.add('is-inside');
    this.drawNav();
    this.drawBody();
  }

  drawNav() {
    const bad = pagesWithErrors(this.pages);
    const icons = this.ctx.config.icons;
    replace(this.nav, this.entries().map((e) => h(`button.set-tab${e.id === this.current && !this.query ? '.is-on' : ''}`, { type: 'button', onclick: () => this.show(e.id) },
      icon(icons[e.id] ?? 'sliders-horizontal'), h('span', e.name), bad.has(e.id) ? h('i.set-dot') : null)));
  }

  /** 右边：搜着的画结果，不然画这一页。 */
  drawBody() {
    const scroll = this.body.scrollTop;
    // 模型的详情挂在右边整栏上：换页、搜索时收掉，模型页自己再挂
    this.panel.querySelector('.set-drawer')?.remove();
    if (this.query.trim()) {
      this.title.textContent = this.ctx.text('search');
      this.drawNav();
      const found = search(this.pages, this.query);
      if (!found.length) {
        replace(this.body, h('p.set-empty', this.ctx.text('no_match', { query: this.query.trim() })));
        return;
      }
      const byPage = new Map();
      for (const f of found) byPage.set(f.page, [...(byPage.get(f.page) ?? []), f.item]);
      replace(this.body, [...byPage].map(([page, items]) => groupBlock(page.name, items.map((item) => coreRow(this, item)))));
      return;
    }
    const entry = this.entries().find((e) => e.id === this.current);
    this.title.textContent = entry?.name ?? '';
    const page = this.pages.find((p) => p.id === this.current);
    let kids;
    if (this.current === 'models') kids = drawModels(this);
    else if (this.current === 'appearance') kids = drawLook(this);
    else if (this.current === 'packages') {
      // 上面一段核心的软件包（一个包一组，照 `config.schema`），下面一段网页自己的组件
      const core = page?.groups.map((g) => groupBlock(g.name, g.items.map((item) => coreRow(this, item)))).filter(Boolean) ?? [];
      kids = [
        ...(page?.problems ?? []).map((p) => banner(p)),
        core.length ? h('section.set-part', h('h2.set-part-name', this.ctx.text('packages_core')), core) : null,
        h('section.set-part', core.length ? h('h2.set-part-name', this.ctx.text('packages_web')) : null, drawPackages(this)),
      ];
    }
    else if (entry?.section) kids = entry.section.render?.(sectionKit(this)) ?? null;
    else if (page) {
      kids = [
        ...page.problems.map((p) => banner(p)),
        ...page.groups.map((g) => groupBlock(g.name, g.items.map((item) => coreRow(this, item)))),
      ];
    }
    replace(this.body, kids);
    this.body.scrollTop = scroll;
  }

  /**
   * 存一项核心的配置（第 6 条）：写在哪一层照 `layerFor`，带上这一层读到的 `expect`。
   * @param {import('./model.js').Item} item
   * @param {{value?: any, input?: string, unset?: true}} change
   * @returns {Promise<string|null>} 拒了的原因（给人看的一句）；成了是 `null`
   */
  save(item, change) {
    const layer = layerFor(item);
    return this.saveMany(layer, [{ key: item.key, ...change, expect: expectFor(item.entry, layer) }]);
  }

  /**
   * 一条 `config.set` 改几项（全收或者全不收）：成了重读重画，拒了交回原因（冲突的先重读，写「别处改过了，现在是 X」）。
   * @param {string} layer
   * @param {{key: string, value?: any, input?: string, unset?: true, expect?: any}[]} changes
   * @returns {Promise<string|null>}
   */
  async saveMany(layer, changes) {
    if (!changes.length) return null;
    try {
      await this.ctx.core.request('config.set', { layer, changes });
    } catch (err) {
      if (err.reason === 'config_conflict' || err.data?.current) {
        await this.reload();
        const now = err.data?.current && 'value' in err.data.current ? JSON.stringify(err.data.current.value) : this.ctx.text('layers.none');
        return this.ctx.text('conflict', { value: now });
      }
      const problems = err.data?.problems ?? [];
      return problems.length ? problems.map((p) => p.message).join('\n') : err.message;
    }
    await this.reload();
    return null;
  }

  /** 重读核心的配置、重画；正在改的那一项有焦点的，等焦点走了再画（第 9 条）。 */
  async reload() {
    await this.load();
    if (this.editing()) {
      this.pending = true;
      return;
    }
    this.drawNav();
    this.drawBody();
  }

  /** 右边有输入框正有焦点、改了还没存。 */
  editing() {
    const el = /** @type {HTMLElement|null} */ (document.activeElement);
    return !!el && this.body.contains(el) && el.matches('input:not([type=checkbox]), textarea') && el.dataset.dirty === '1';
  }

  /** 焦点走了：攒着的重画补上。 */
  flush() {
    if (!this.pending || this.editing()) return;
    this.pending = false;
    this.drawNav();
    this.drawBody();
  }

  /** 框底下提示一句，停 2 秒（照「提示」）。 */
  toast(text) {
    this.panel.querySelector('.set-toast')?.remove();
    const el = h('div.set-toast', { role: 'status' }, text);
    this.panel.append(el);
    setTimeout(() => leave(el, () => el.remove()), 2000);
  }

  /** 别处改了配置（别的终端、命令行、手改文件）：重读重画。 */
  onPush(method) {
    if (!this.isOpen || method !== 'config.changed') return;
    clearTimeout(this.pushTimer);
    this.pushTimer = setTimeout(() => this.reload().catch(() => {}), 80);
  }

  /** `Esc`：先关框里开着的菜单、详情，再关弹窗；焦点关在框里（`Tab` 不跑到后面）。 */
  onKey(/** @type {KeyboardEvent} */ e) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      e.preventDefault();
      const open = this.panel.querySelector('.set-menu, .set-drawer, .set-form, .set-pool.is-editing');
      if (open) {
        open.dispatchEvent(new CustomEvent('set-dismiss'));
        return;
      }
      if (this.query) {
        this.query = '';
        this.search.value = '';
        this.drawNav();
        this.drawBody();
        return;
      }
      this.close();
      return;
    }
    if (e.key === 'Tab') {
      const focusable = [...this.panel.querySelectorAll('button, input, select, [tabindex="0"]')].filter((el) => /** @type {HTMLElement} */ (el).offsetParent);
      if (!focusable.length) return;
      const first = /** @type {HTMLElement} */ (focusable[0]);
      const last = /** @type {HTMLElement} */ (focusable.at(-1));
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
  }
}
