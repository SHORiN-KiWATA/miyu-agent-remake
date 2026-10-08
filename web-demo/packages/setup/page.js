// @ts-check
//! 设置页的「人格」「预设」两页（蓝图 `web.md`「人格、预设、工作区」第 6 条；挂进设置页的 `settings.section`）：打开时先重读列表和默认的
//! （通用页刚改过默认的这里跟着变），一个一块：名字、说明，默认的那个标「默认」；文件写错的那一块写原因（照 `check` 给人看的那一句、
//! 第几行、哪个文件，项目主人 2026-10-08 定写中文）。照「用户来这一页要做什么」画（2026-10-08 项目主人：来自哪一层、以谁为底、编号、
//! 分语言的名字都是核心怎么存，用户用不上，不显示）。
//! 点一块在原地展开详情（`persona-editor.js`、`preset-editor.js`），一次开一个；顶上「＋ 新建」先只填名字（`form.js` 的 `nameFirst`），
//! 建好接着在详情里写。一页是一个常驻的对象：设置页别处改了配置重画、关了再开，交回的是同一块，开着的详情和没存的字都还在。

import { h, replace } from '../../src/lib/dom.js';
import { personaName, presetName } from './model.js';
import { nameFirst } from './form.js';
import { PersonaEditor, refusalText } from './persona-editor.js';
import { PresetEditor } from './preset-editor.js';

/** 一处问题写成一句。 @param {any} ctx @param {{line: number|null, message: string, file: string}} p */
export function problemText(ctx, p) {
  if (!p.file) return p.message;
  return p.line != null ? ctx.text('page.problem_line', { file: p.file, line: p.line, message: p.message }) : ctx.text('page.problem_file', { file: p.file, message: p.message });
}

/** 写错的一块：编号（读不出名字），下面一处一行「persona.toml 第 3 行：哪里错了」。 */
function badCard(ctx, id, problems) {
  return h('div.setup-card.is-bad', h('h4', id), problems.map((p) => h('p.setup-card-problem', problemText(ctx, p))));
}

/** 人格页、预设页不一样的那几处。 */
const KINDS = {
  persona: {
    list: (c) => c.personas, fallback: (c) => c.personaDefault, name: personaName, none: 'page.none', add: 'edit.new_persona',
    create: (core, name) => core.request('persona.set', { changes: [{ key: 'persona.name', value: name }] }).then((r) => r?.persona),
    editor: (ctx, kit, catalog, id, hooks) => new PersonaEditor(ctx, kit, id, hooks),
  },
  preset: {
    list: (c) => c.presets, fallback: (c) => c.presetDefault ?? 'full', name: presetName, none: 'presets.none', add: 'edit.new_preset',
    create: (core, name) => core.request('preset.set', { changes: [{ key: 'preset.name', value: name }] }).then((r) => r?.preset),
    editor: (ctx, kit, catalog, id, hooks) => new PresetEditor(ctx, kit, catalog, id, hooks),
  },
};

export class ListPage {
  /** @param {any} ctx @param {import('./catalog.js').Catalog} catalog @param {'persona'|'preset'} kind */
  constructor(ctx, catalog, kind) {
    this.ctx = ctx;
    this.catalog = catalog;
    this.kind = kind;
    this.k = KINDS[kind];
    this.el = h('div.setup-cards', h('p.setup-empty', ctx.text('page.loading')));
    /** @type {{id: string, editor: PersonaEditor|PresetEditor}|null} 开着的详情 */
    this.open = null;
    /** @type {HTMLElement|null} 新建时只填名字的那一块 */
    this.creating = null;
    /** @type {import('./form.js').Kit|null} */
    this.kit = null;
  }

  /** 设置页画这一页：重读列表、交回同一块。 @param {import('./form.js').Kit} kit */
  render(kit) {
    this.kit = kit;
    this.refresh();
    return this.el;
  }

  async refresh() {
    try {
      await this.catalog.load();
    } catch (err) {
      replace(this.el, h('p.setup-empty.is-bad', refusalText(err)));
      return;
    }
    this.draw();
  }

  draw() {
    const t = (key, fields) => this.ctx.text(key, fields);
    const list = this.k.list(this.catalog) ?? [];
    const fallback = this.k.fallback(this.catalog);
    // 开着的那一个没了（别处删了）：收起
    if (this.open && !list.some((p) => p[this.kind] === this.open?.id)) this.open = null;
    const cards = list.map((p) => {
      const id = p[this.kind];
      if (p.problem) return badCard(this.ctx, id, this.catalog.problemOf(this.kind, p));
      if (this.open?.id === id) return this.open.editor.el;
      const card = h('div.setup-card.is-clickable', { tabindex: '0', role: 'button', onclick: () => this.show(id) },
        h('h4', this.k.name(p)), p.summary ? h('p', p.summary) : null,
        id === fallback ? h('div.setup-tags', h('span.setup-tag', t('page.default'))) : null);
      card.addEventListener('keydown', (e) => {
        if (e.key !== 'Enter' && e.key !== ' ') return;
        e.preventDefault();
        this.show(id);
      });
      return card;
    });
    const add = h('div.setup-bar', h('button.setup-new', { type: 'button', onclick: () => this.startNew() }, t(this.k.add)));
    replace(this.el, add, this.creating, cards.length ? cards : h('p.setup-empty', t(this.k.none)));
  }

  /** 展开一个：开着的那个改了没存的不换，提示先存或者取消。 @param {string} id */
  async show(id) {
    if (!this.kit || this.open?.id === id) return;
    if (this.open?.editor.dirty()) {
      this.open.editor.tryClose();
      return;
    }
    const editor = this.k.editor(this.ctx, this.kit, this.catalog, id, {
      saved: () => this.refresh(),
      removed: (remains) => {
        // 恢复出厂的还在，照出厂的样子重读；删掉的收起
        if (remains) editor.load();
        else this.open = null;
        this.refresh();
      },
      close: () => {
        this.open = null;
        this.draw();
      },
    });
    this.open = { id, editor };
    this.draw();
    await editor.load();
  }

  /** 「＋ 新建」：顶上一块只填名字；建好收起它、展开新的那一个接着写。 */
  startNew() {
    if (!this.kit || this.creating) return;
    if (this.open?.editor.dirty()) {
      this.open.editor.tryClose();
      return;
    }
    const t = (key) => this.ctx.text(key);
    this.creating = nameFirst(this.kit, t, t(this.k.add), t('edit.name_hint'), async (name) => {
      let id;
      try {
        id = await this.k.create(this.ctx.core, name);
      } catch (err) {
        return refusalText(err);
      }
      this.creating = null;
      await this.catalog.load();
      this.open = null;
      await this.show(id);
      this.open?.editor.focusFirst();
      return null;
    }, () => {
      this.creating = null;
      this.draw();
    });
    this.draw();
  }
}
