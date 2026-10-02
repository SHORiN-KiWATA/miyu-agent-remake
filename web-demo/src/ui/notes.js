// @ts-check
//! 不挂在她头下的一行（蓝图 `web.md`「后台命令、子代理的回报」「压缩、清空」「回顾」）：记号有颜色、字暗，和收尾那一行一样对齐、
//! 一样大。回报那一行能点开：照时间线点开一步的样子，下面铺底色、分段——后台命令是命令和整份输出（blob 经宿主的地址取，
//! 太长的只画最后几行），子代理是交回的正文（Markdown）。点开、收起时钉住被点的那一行（`follow.js`，和时间线同一个事件）。

import { h, icon } from './dom.js';
import { res, t } from '../util/res.js';
import { blobUrl } from '../core/host.js';
import { renderMarkdown } from '../markdown/render.js';

/**
 * @param {any} it 条目（`model/notes.js`）
 * @param {{session: string|null}} where 取输出照哪个会话
 * @param {{say: (text: string, good?: boolean) => void, hooks: (scope: any) => any}} markdown 画子代理的报告（对话区的那一套）
 */
export function noteNode(it, where, markdown) {
  // 回顾：一张卡片，头一行图标和「回顾」，下面是回顾的字（蓝图「回顾」第 2 条）
  if (it.recap != null) {
    return h('div.note.is-recap', h('div.note-recap', h('div.note-recap-head', icon('file-text'), h('span', t('notes.recap'))), h('div.note-recap-text', it.recap)));
  }
  const line = h(`div.note-line.tone-${it.tone}`, it.mark ? h('span.note-mark', it.mark) : null, h('span.note-text', it.text));
  const node = h('div.note', line);
  if (!it.detail) return node;
  // 右边的小箭头：悬停才露，点开以后一直露着、转成朝下（和时间线一个规矩）
  line.append(h('span.tl-chevron', icon('chevron-right')));
  node.classList.add('is-expandable');
  line.setAttribute('role', 'button');
  line.setAttribute('tabindex', '0');
  line.setAttribute('aria-expanded', 'false');
  // 点开的放在和时间线一样的收放的壳里：高度从 0 长出来、收回去，淡入淡出（蓝图「后台命令、子代理的回报」，展开收起一个设计）
  const inner = h('div.tl-fold-inner');
  node.append(h('div.tl-fold', inner));
  node.style.setProperty('--tl-step', `${res.timeline.step_ms}ms`);
  const toggle = () => {
    line.dispatchEvent(new CustomEvent('tl-toggle', { bubbles: true }));
    const open = !node.classList.contains('is-open');
    // 第一次点开才取内容（命令的输出要经宿主取一次）；收起时内容留着，收的动画里还看得到
    if (open && !inner.firstChild) inner.append(detailNode(it.detail, where, markdown));
    node.classList.toggle('is-open', open);
    line.setAttribute('aria-expanded', String(open));
  };
  line.addEventListener('click', toggle);
  // 点展开出来的那一块也收起：点的是链接、按钮、代码块，或者拖选了字的不算
  inner.addEventListener('click', (e) => {
    if (!node.classList.contains('is-open') || String(getSelection() ?? '')) return;
    if (/** @type {Element} */ (e.target).closest('a, button, input, textarea, pre, .code-block')) return;
    toggle();
  });
  line.addEventListener('keydown', (e) => {
    if (e.key !== 'Enter' && e.key !== ' ') return;
    e.preventDefault();
    toggle();
  });
  return node;
}

/**
 * 点开看的，照时间线点开一步的样子分段（`.tl-body`、`.tl-detail`）：后台命令是「命令」「输出」两段（输出第一次点开才取），
 * 子代理是交回的正文，照她的回答画 Markdown。
 */
function detailNode(detail, where, markdown) {
  const n = res.text.notes;
  const section = (label, ...body) => h('div.tl-detail', h('div.tl-label', label), ...body);
  if (detail.kind === 'text') {
    const md = h('div.reply.markdown-body.note-report');
    renderMarkdown(md, detail.text, { say: markdown.say, hooks: markdown.hooks('note'), streaming: false });
    return h('div.tl-body.note-detail', h('div', md, detail.truncated ? h('div.note-detail-head', n.truncated) : null));
  }
  const command = detail.command ? section(n.command_label, h('pre', detail.command)) : null;
  if (!detail.hash) return h('div.tl-body.note-detail', command, section(n.output_label, h('div.note-detail-head', n.no_output)));
  const pre = h('pre', n.loading);
  const head = h('div.note-detail-head', { hidden: true });
  fetch(blobUrl(detail.hash, 'text/plain'))
    .then((r) => (r.ok ? r.text() : Promise.reject(new Error(`${r.status} ${r.statusText}`))))
    .then((text) => {
      const lines = text.replace(/\n$/, '').split('\n');
      const tail = res.layout.report_tail_lines;
      if (lines.length > tail) {
        head.textContent = t('notes.tail', { count: tail });
        head.hidden = false;
      }
      pre.textContent = lines.slice(-tail).join('\n');
    })
    .catch((err) => { pre.textContent = t('notes.load_failed', { reason: err.message }); });
  return h('div.tl-body.note-detail', command, section(n.output_label, head, pre));
}
