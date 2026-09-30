// @ts-check
//! 附件（软件包 `attachments`，蓝图 `web.md`「附件」）：输入框里放图片和文件，跟着话一起发。
//!
//! - 放进来：回形针按钮（挂进 `composer.bar`，只在用手指点的设备上露，`style.css`）打开选文件；把文件拖进窗口（整页盖一层，正中一个文件加号和一句话，在哪松开都收，
//!   照 Claude 网页端）；在输入框里粘贴文件。
//!   选文件、拖放、缩略图都经宿主（服务 `host` 的 `files`，蓝图 `web/architecture.md`「宿主」）：浏览器给内容，桌面端给路径。
//! - 传：宿主把它变成核心读得到的路径（浏览器交给桥存进临时目录，桌面端本来就有路径），再 `blob.put`；桥存的那一份存好了
//!   `web.upload_done` 让桥删掉。
//! - 框里那一排挂进 `composer.head`（`tray.js`）；发的时候经 `composer.payload` 交出去，核心拒了放回来（`model.js` 的 `Tray`）；
//!   记进输入历史的是核心存好的那一份，翻出来的照它回到框里（蓝图「输入历史」）。
//!
//! 停用了按钮、那一排、拖放都没了，框里没发的附件丢掉。

import { h, icon } from '../../src/lib/dom.js';
import { show, hide } from '../../src/lib/motion.js';
import { Tray, admit, mediaType } from './model.js';
import { TrayView } from './tray.js';

/** @param {any} ctx */
export function apply(ctx) {
  const t = (path, fields) => ctx.text(path, fields);
  const tray = new Tray();
  const files = ctx.host.files;
  const view = new TrayView(tray, t, (id) => tray.remove(id), files, ctx.config);
  ctx.effect(() => () => view.destroy());
  ctx.effect(() => tray.watch(() => ctx.composer.changed()));

  /** 传一个：宿主给出路径、核心存好；出错的那一块拿掉，提示一句。桥存的那一份总是删掉（本来就有路径的不删）。 */
  const put = async (/** @type {import('./model.js').Item} */ item) => {
    let path = null;
    const own = !!item.file.path;
    try {
      path = await files.stage(item.file);
      const type = mediaType(item.file.type);
      const got = await ctx.core.request('blob.put', type ? { path, media_type: type } : { path });
      tray.ready(item.id, got);
    } catch (err) {
      if (tray.remove(item.id)) ctx.composer.say(t('failed', { name: item.name, reason: reasonOf(err, t) }));
    } finally {
      if (path && !own) ctx.core.request('web.upload_done', { path }).catch(() => {});
    }
  };

  /** 放进来几个：超大的、放不下的不收，提示；收下的一个个传。 */
  const take = (/** @type {import('./model.js').FileLike[]} */ list) => {
    if (!list.length) return;
    const r = admit(tray.items.length, list, ctx.config);
    for (const f of r.tooBig) ctx.composer.say(t('too_big', { name: f.name, mib: ctx.config.max_mib }));
    if (r.tooMany) ctx.composer.say(t('too_many', { max: ctx.config.max_files }));
    for (const f of r.accepted) put(tray.add(f));
    ctx.composer.focus();
  };

  // 回形针：打开系统的选文件，能多选
  const button = h('button.icon-button.attach-button', { type: 'button', title: t('attach'), 'aria-label': t('attach'), onclick: async () => take(await files.pick()) }, icon('paperclip'));
  ctx.slots.mount('composer.bar', { id: 'attachments', order: 10, render: () => h('span.attach-tools', button) });
  ctx.slots.mount('composer.head', { id: 'attachments', order: 10, render: () => view.el });
  ctx.slots.mount('composer.payload', {
    id: 'attachments',
    order: 10,
    render: () => null,
    has: () => tray.has(),
    busy: () => tray.busy(),
    take: () => tray.take(),
    putBack: (given) => tray.putBack(given),
    // 输入历史（蓝图「输入历史」第 1、2 条）：交出去的记成核心存好的那一份；翻出来的换上、走回没发的那句拿掉；改了字留下
    keep: (given) => tray.keep(given),
    recall: (saved) => tray.recall(saved?.kept ?? null, saved?.session ?? null),
    settle: () => tray.settle(),
  });

  // 粘贴：粘的是文件（截图）的收下，是字的照旧
  const onPaste = (/** @type {ClipboardEvent} */ e) => {
    const pasted = files.refs(e.clipboardData?.files ?? []);
    if (!pasted.length) return;
    e.preventDefault();
    take(pasted);
  };
  const input = ctx.composer.input;
  input.addEventListener('paste', onPaste);
  ctx.effect(() => () => input.removeEventListener('paste', onPaste));

  // 拖进来（第 1 条）：文件一拖进窗口，整页盖一层（页面淡成几乎看不清），正中一个文件加号和一句话；在哪松开都收（宿主分辨拖的
  // 是不是文件）；拖出窗口、松开以后淡出。这一层挂在 `body` 上盖住整页，不接鼠标
  const layer = h('div.attach-drop', { hidden: true }, h('div.attach-drop-card', icon('file-plus'), h('span.attach-drop-text', t('drop_here'))));
  document.body.append(layer);
  const unwatch = files.watchDrop(document.body, {
    enter: () => show(layer),
    over: () => {},
    leave: () => hide(layer),
    drop: take,
  });
  ctx.effect(() => () => {
    unwatch();
    layer.remove();
  });
}

/** 出错的原因：核心拒的照原因码查这个包的字，查不到的、桥不收的写原话。 */
function reasonOf(err, t) {
  const code = err?.reason;
  const known = code ? t(`reasons.${code}`) : null;
  return known && known !== `reasons.${code}` ? known : err?.message ?? String(err);
}
