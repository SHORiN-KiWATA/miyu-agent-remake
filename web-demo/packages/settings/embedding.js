// @ts-check
//! 「模型」页的「按意思找记忆」（配置项 `models.embedding`，记忆那一步 R-5 补的 `model_or`）：照「默认文本模型」那一页的样子，一列单选。
//! 最上面一行核心写的说明；先是 `options` 里的几样（本机的模型、关），没写的照「本机的模型」；最后一项「指定模型」，同一行里一个框写
//! `<供应商>/<模型>`，回车或「用这个」。不列对话模型的列表：对话模型做不了这件事，列出来是误导，嵌入模型多半也不在那份列表里。
//! 写错的照核心的话写在下面（`bad_format`），指的供应商没配的照核心的问题黄字提示（`bad_reference`，只报不丢）。

import { h } from '../../src/lib/dom.js';
import { plainItem } from './model.js';

/** @param {any} dialog */
export function drawEmbedding(dialog) {
  const ctx = dialog.ctx;
  const t = (k, f) => ctx.text(`models.embedding.${k}`, f);
  const item = plainItem(dialog.schema, dialog.got, 'models.embedding');
  if (!item) return null;
  const written = item.entry?.value ?? null;
  // 没写的照「本机的模型」（核心：不写就是 local）
  const current = typeof written === 'string' ? written : 'local';
  const options = item.options ?? [];
  const custom = !options.some((o) => o.value === current);
  const note = h('p.set-error', { hidden: true });
  const pick = async (value) => {
    if (value === current && written !== null) return;
    const why = await dialog.save(item, { value });
    if (why) {
      note.hidden = false;
      note.textContent = why;
    }
  };
  const radio = (on, label, onclick) => h(`button.set-pick${on ? '.is-on' : ''}`, { type: 'button', onclick }, h('i.set-radio'), h('span.set-model-name', label));
  const field = /** @type {HTMLInputElement} */ (h('input.set-input', { type: 'text', placeholder: t('manual_hint'), value: custom ? current : '', spellcheck: 'false', autocomplete: 'off' }));
  const use = () => {
    const v = field.value.trim();
    if (v) pick(v);
    else field.focus();
  };
  field.addEventListener('keydown', (e) => {
    if (e.key !== 'Enter' || e.isComposing) return;
    e.preventDefault();
    use();
  });
  const manual = h(`div.set-pick.set-manual${custom ? '.is-on' : ''}`, { onclick: (e) => { if (e.target === e.currentTarget) field.focus(); } },
    h('i.set-radio'), h('span.set-model-name', t('pick')), field, h('button.set-btn', { type: 'button', onclick: use }, t('manual_use')));
  return [
    item.description ? h('p.set-desc.set-embed-desc', item.description) : null,
    h('div.set-picks', ...options.map((o) => radio(current === o.value, o.name, () => pick(o.value))), manual),
    ...(item.problems ?? []).map((p) => h(`p.set-problem.is-${p.level === 'error' ? 'error' : 'warn'}`, p.message)),
    note,
  ];
}
