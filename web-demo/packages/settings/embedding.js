// @ts-check
//! 「模型」页的「语义模型」（配置项 `models.embedding`，记忆那一步 R-5 补的 `model_or`；2026-10-09 项目主人定名「语义模型」）：一列单选，
//! 照「默认文本模型」那一页的样子。最上面一行核心写的说明；单选照 `options`（内置模型、关），没写的照内置模型；有 `note` 的（内置模型的
//! 模型名）接在名字后面暗色写。「指定一家供应商的模型」等核心给出能做嵌入的模型清单再加：手写「供应商/模型」人不知道写什么
//! （项目主人：「根本不可用」），先不放。写错的照核心的话写在下面，指的供应商没配的照核心的问题提示。

import { h } from '../../src/lib/dom.js';
import { plainItem } from './model.js';

/** @param {any} dialog */
export function drawEmbedding(dialog) {
  const item = plainItem(dialog.schema, dialog.got, 'models.embedding');
  if (!item) return null;
  const written = item.entry?.value ?? null;
  // 没写的照内置模型（核心：不写就是 local）
  const current = typeof written === 'string' ? written : 'local';
  const note = h('p.set-error', { hidden: true });
  const pick = async (value) => {
    if (value === current && written !== null) return;
    const why = await dialog.save(item, { value });
    if (why) {
      note.hidden = false;
      note.textContent = why;
    }
  };
  // `available: false` 的（内置模型的包没装，核心 F-5 再补）：暗着、点不了，后面写「没装」
  const options = (item.options ?? []).map((o) => {
    const off = o.available === false;
    return h(`button.set-pick${current === o.value ? '.is-on' : ''}${off ? '.is-off' : ''}`, { type: 'button', disabled: off, onclick: () => pick(o.value) },
      h('i.set-radio'), h('span.set-model-name', o.name), o.note ? h('code.set-model-id', o.note) : null, off ? h('span.set-pick-provider', dialog.ctx.text('models.not_installed')) : null);
  });
  // 现在写的是一家供应商的模型（以前在别处写的）：照写着显示成选中的一行，人能换回上面的
  const custom = !(item.options ?? []).some((o) => o.value === current)
    ? h('button.set-pick.is-on', { type: 'button' }, h('i.set-radio'), h('span.set-model-name', current)) : null;
  // 选着的是用不了的（内置模型没装）：说清现在怎样（设计 30 第二节：没装内置模型就只有关键词那一路）
  const using = (item.options ?? []).find((o) => o.value === current);
  const missing = using?.available === false ? h('p.set-problem.is-warn', dialog.ctx.text('models.embedding_missing', { name: using.name })) : null;
  return [
    item.description ? h('p.set-desc.set-embed-desc', item.description) : null,
    h('div.set-picks', options, custom),
    missing,
    ...(item.problems ?? []).map((p) => h(`p.set-problem.is-${p.level === 'error' ? 'error' : 'warn'}`, p.message)),
    note,
  ];
}
