// @ts-check
//! 直接在页面上开一个人格的编辑卡片（左栏左上角的头像点进来的，2026-10-10 项目主人：直接打开编辑卡片，不先开设置页）：和设置页「人格」页
//! 里点一块开的是同一张卡片、同一个编辑器（`float.js`、`persona-editor.js`），只是盖在整页上。控件照设置页的（服务 `settings` 的 `kit`）。
//! 点卡片外面、`Esc`、✕ 交给编辑器（改了没存的不关、提示一句）；存了重读人格列表，左上角、对话区的头像跟着换。

import { floatCard } from './float.js';
import { PersonaEditor } from './persona-editor.js';
import { personaName } from './model.js';

/** 开着的那一张：同一时刻只开一张 @type {{close: () => void}|null} */
let opened = null;

/**
 * @param {any} ctx `setup` 包的 ctx
 * @param {import('./catalog.js').Catalog} catalog
 * @param {string} id 人格的编号
 * @param {(panel: HTMLElement, toast: (text: string) => void) => import('./form.js').Kit} kitOf 服务 `settings` 的 `kit`
 */
export async function editPersona(ctx, catalog, id, kitOf) {
  if (opened) return;
  const t = (/** @type {string} */ key) => ctx.text(key);
  /** @type {PersonaEditor|null} */
  let editor = null;
  const dismiss = () => (editor ? editor.tryClose() : close());
  const card = floatCard({ title: '', body: document.createElement('div'), close: t('edit.close'), onDismiss: dismiss });
  const close = () => {
    if (opened !== handle) return;
    opened = null;
    card.close();
  };
  const handle = { close };
  opened = handle;
  card.el.classList.add('is-page');
  // `Esc`：开着的下拉先收（同设置页），没有的交给编辑器；不让它冒到输入框那边（两下 `Esc` 是打断）
  card.el.addEventListener('keydown', (e) => {
    if (e.key !== 'Escape') return;
    e.stopPropagation();
    const menu = card.el.querySelector('.set-menu');
    if (menu) menu.dispatchEvent(new CustomEvent('set-dismiss'));
    else dismiss();
  });
  document.body.append(card.el);
  const box = /** @type {HTMLElement} */ (card.el.querySelector('.setup-float'));
  box.tabIndex = -1;
  box.focus({ preventScroll: true });
  const item = catalog.personas?.find((p) => p.persona === id);
  editor = new PersonaEditor(ctx, kitOf(card.el, (text) => ctx.composer.say(text)), catalog, id, {
    saved: () => catalog.load(),
    removed: (/** @type {boolean} */ remains) => {
      // 恢复出厂的还在，照出厂的样子重读；删掉的关掉卡片
      if (remains) editor?.load();
      else close();
      catalog.load();
    },
    close,
  }, item ? { name: personaName(item) } : {});
  card.setTitle(editor.title);
  card.setBody(editor.body);
  await editor.load();
}
