// @ts-check
//! 设置页的「人格」「预设」两页（蓝图 `web.md`「人格、预设、工作区」第 6 条；挂进设置页的 `settings.section`）：打开时先重读列表和默认的
//! （通用页刚改过默认的这里跟着变），一个一块：名字、说明，默认的那个标「默认」；文件写错的那一块写原因（照 `check` 给人看的那一句、
//! 第几行、哪个文件，项目主人 2026-10-08 定写中文）。照「用户来这一页要做什么」画（2026-10-08 项目主人：来自哪一层、以谁为底、编号、
//! 分语言的名字都是核心怎么存，用户用不上，不显示）。
//! 点一块在原地展开详情（`persona-editor.js`、`preset-editor.js`），一次开一个；顶上「＋ 新建」先只填名字（`form.js` 的 `nameFirst`），
//! 建好接着在详情里写。一页是一个常驻的对象：设置页别处改了配置重画、关了再开，交回的是同一块，开着的详情和没存的字都还在。

import { h, icon, replace, hasIcon } from '../../src/lib/dom.js';
import { personaName, presetName } from './model.js';
import { nameFirst } from './form.js';
import { floatCard } from './float.js';
import { PersonaEditor, refusalText } from './persona-editor.js';
import { PresetEditor } from './preset-editor.js';

/** 一处问题写成一句。 @param {any} ctx @param {{line: number|null, message: string, file: string}} p */
export function problemText(ctx, p) {
  if (!p.file) return p.message;
  return p.line != null ? ctx.text('page.problem_line', { file: p.file, line: p.line, message: p.message }) : ctx.text('page.problem_file', { file: p.file, message: p.message });
}

/**
 * 人格的头像、预设的图标（核心 P-5）：人格有头像的画图（读到以前先画名字的第一个字），预设写了认得的图标的画图标；别的画名字的第一个字。
 * @param {import('./catalog.js').Catalog} catalog @param {'persona'|'preset'} kind @param {any} p 列表里那一项 @param {string} name
 */
export function faceOf(catalog, kind, p, name) {
  const face = (/** @type {any} */ inner) => h(`span.setup-face.is-${kind}`, { 'aria-hidden': 'true' }, inner);
  if (kind === 'persona') {
    const url = catalog.avatars.url(p.persona, p.avatar);
    return url ? face(h('img', { src: url, alt: '' })) : face([...name][0] ?? '');
  }
  return p.icon && hasIcon(p.icon) ? face(icon(p.icon)) : face([...name][0] ?? '');
}

/** 写错的一块：编号（读不出名字），下面一处一行「persona.toml 第 3 行：哪里错了」。 */
function badCard(ctx, id, problems) {
  return h('div.setup-card.is-bad', h('h4', id), problems.map((p) => h('p.setup-card-problem', problemText(ctx, p))));
}

/** 人格页、预设页不一样的那几处。 */
const KINDS = {
  persona: {
    list: (c) => c.personas, fallback: (c) => c.personaDefault, name: personaName, none: 'page.none', add: 'edit.new_persona', intro: 'page.intro', summary: true,
    create: (core, name) => core.request('persona.set', { changes: [{ key: 'persona.name', value: name }] }).then((r) => r?.persona),
    editor: (ctx, kit, catalog, id, hooks, look) => new PersonaEditor(ctx, kit, catalog, id, hooks, look),
  },
  preset: {
    // 预设不写说明（2026-10-08 项目主人：没什么意义，名字已经说清了）
    list: (c) => c.presets, fallback: (c) => c.presetDefault ?? 'full', name: presetName, none: 'presets.none', add: 'edit.new_preset', intro: null, summary: false,
    create: (core, name) => core.request('preset.set', { changes: [{ key: 'preset.name', value: name }] }).then((r) => r?.preset),
    editor: (ctx, kit, catalog, id, hooks, look) => new PresetEditor(ctx, kit, catalog, id, hooks, look),
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
    /** @type {{id: string|null, editor: PersonaEditor|PresetEditor|null, card: ReturnType<typeof floatCard>}|null} 开着的浮卡（新建时 `id` 是空的） */
    this.open = null;
    /** @type {import('./form.js').Kit|null} */
    this.kit = null;
    /** 画出来以后要打开的那一个（`focus`） @type {string|null} */
    this.want = null;
    // 人格的头像读到了：列表在页面上的重画（头像在卡片上）
    if (kind === 'persona') catalog.listeners.add(() => { if (this.el.isConnected && this.kit) this.draw(); });
  }

  /**
   * 这一页画出来（或者已经画着）以后打开这个人格、预设的编辑卡片（左上角的头像点进来的）；`null` 只开这一页。开着别的卡片的不换。
   * @param {string|null} id
   */
  focus(id) {
    this.want = id;
    if (this.el.isConnected && this.kit) this.refresh();
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
    const want = this.want;
    this.want = null;
    if (want && (this.k.list(this.catalog) ?? []).some((p) => p[this.kind] === want && !p.problem)) this.show(want);
  }

  draw() {
    const t = (key, fields) => this.ctx.text(key, fields);
    const list = this.k.list(this.catalog) ?? [];
    const fallback = this.k.fallback(this.catalog);
    const cards = list.map((p) => {
      const id = p[this.kind];
      if (p.problem) return badCard(this.ctx, id, this.catalog.problemOf(this.kind, p));
      // 「默认」接在名字后面，不另占一行（2026-10-08 项目主人）；左边人格的头像、预设的图标（`faceOf`）
      // （2026-10-10 项目主人：长条里只有一点字不合理，改成卡片网格）
      const name = this.k.name(p);
      const card = h('div.setup-card.is-clickable', { tabindex: '0', role: 'button', onclick: () => this.show(id) },
        faceOf(this.catalog, this.kind, p, name),
        h('div.setup-card-text',
          h('h4', h('span.setup-card-name', name), id === fallback ? h('span.setup-tag', t('page.default')) : null),
          this.k.summary && p.summary ? h('p', p.summary) : null));
      card.addEventListener('keydown', (e) => {
        if (e.key !== 'Enter' && e.key !== ' ') return;
        e.preventDefault();
        this.show(id);
      });
      return card;
    });
    // 顶上一行：这一页管什么（人格页，2026-10-08 项目主人），右边「＋ 新建」
    const add = h('div.setup-bar', this.k.intro ? h('p.setup-intro', t(this.k.intro)) : null, h('button.setup-new', { type: 'button', onclick: () => this.startNew() }, icon('plus'), h('span', t(this.k.add))));
    replace(this.el, add, cards.length ? h('div.setup-grid', cards) : h('p.setup-empty', t(this.k.none)));
  }

  /** 点卡片外面、`Esc`、✕：开着编辑器的交给它（人格改了没存的不关、提示一句），新建只填名字那一步直接关。 */
  dismiss() {
    if (this.open?.editor) this.open.editor.tryClose();
    else this.closeCard();
  }

  /** 关掉开着的浮卡（淡出），列表照旧。 */
  closeCard() {
    this.open?.card.close();
    this.open = null;
  }

  /** 给一个人格、预设开编辑器（放进 `card`；没给的新开一张浮卡）。 @param {string} id @param {ReturnType<typeof floatCard>|null} [card] */
  editorFor(id, card = null) {
    const item = (this.k.list(this.catalog) ?? []).find((p) => p[this.kind] === id);
    const editor = this.k.editor(this.ctx, /** @type {any} */ (this.kit), this.catalog, id, {
      saved: () => this.refresh(),
      removed: (remains) => {
        // 恢复出厂的还在，照出厂的样子重读；删掉的关掉卡片
        if (remains) editor.load();
        else this.closeCard();
        this.refresh();
      },
      close: () => { if (this.open?.editor === editor) this.closeCard(); },
    }, item ? { name: this.k.name(item) } : {});
    if (card) {
      card.setTitle(editor.title);
      card.setBody(editor.body);
    } else {
      card = floatCard({ title: editor.title, body: editor.body, close: this.ctx.text('edit.close'), onDismiss: () => this.dismiss() });
      card.mount(this.el);
    }
    this.open = { id, editor, card };
    return editor;
  }

  /** 点一块：开一张浮卡编辑它（2026-10-09 项目主人：原地展开不好，改成悬浮卡片）。 @param {string} id */
  async show(id) {
    if (!this.kit || this.open) return;
    await this.editorFor(id).load();
  }

  /** 「＋ 新建」：浮卡里先只填名字；建好了同一张卡片接着换成这个人格、预设的详情。 */
  startNew() {
    if (!this.kit || this.open) return;
    const t = (key) => this.ctx.text(key);
    const card = floatCard({ title: t(this.k.add), body: h('div'), close: t('edit.close'), onDismiss: () => this.dismiss() });
    card.setBody(nameFirst(this.kit, t, t('edit.name_hint'), async (name) => {
      let id;
      try {
        id = await this.k.create(this.ctx.core, name);
      } catch (err) {
        return refusalText(err);
      }
      await this.catalog.load();
      this.draw();
      // 同一张卡片换成详情，接着写
      const editor = this.editorFor(id, card);
      await editor.load();
      editor.focusFirst();
      return null;
    }, () => this.closeCard()));
    card.mount(this.el);
    this.open = { id: null, editor: null, card };
  }
}
