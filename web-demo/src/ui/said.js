// @ts-check
//! 你说的话和她一轮末尾的按钮（蓝图 `web.md`「对话区」的「你的话的按钮」「编辑」「她的一轮的按钮」，照 Claude 网页端）。
//!
//! - 别人说的（别的账号、子代理、别的 harness、平台上的人，蓝图「不是你说的话」）：一样的气泡，上面一行小字写是谁，没有编辑。
//! - 你的话：悬停时气泡右下角下面出复制、编辑；编辑在气泡里直接改，改完这一轮照新的话重来。带的附件在气泡上面一排
//!   （蓝图「附件」第 6 条）：图片点开是灯箱，文件点了下载；只有附件、没有字的没有气泡，也没有复制。
//! - 她的一轮：收尾那一行后面一排复制、重做。
//! - 编辑、重做只有最新的那一轮有，在回答时没有：由对话区给节点打 `is-latest`、给整块打 `is-running`，CSS 藏。
//!   做什么由整页给（`app.js`），这里只画、接键。

import { h, icon } from './dom.js';
import { res, t } from '../util/res.js';
import { leave } from '../lib/motion.js';
import { imageCard } from './media.js';
import { blobUrl } from '../core/host.js';
import { shortSession } from '../model/words.js';

/**
 * @typedef {{copy: (text: string) => void, edit: (text: string) => void, copyTurn: (turn: number) => void, redo: () => void, openJobs: () => void, openSession: (id: string) => void, compacted?: (session: string|null) => void}} Actions
 *   复制一段字；照改过的话重做最新的一轮；复制她这一轮说的全部正文；原样重做最新的一轮；打开后台任务的浮层；看另一个会话
 * @typedef {{session: string|null, lightbox: () => any, mine?: boolean, titleOf?: (id: string) => string|null}} Media
 *   附件的地址照哪个会话取、现在的灯箱、是不是你说的、一个会话的标题（「来自「标题」」用）
 * @typedef {{kind: 'image'|'file', blob: string, media_type: string, width: number|null, height: number|null, name: string|null}} Attachment
 */

/**
 * 你说的一句话：附件一排、靠右的气泡，下面悬停才露的按钮。
 * @param {{text: string, attachments?: Attachment[], speaker?: {kind: string, name: string, id?: string}}} it
 * @param {Actions} on
 * @param {Media} media
 */
export function userNode(it, on, media) {
  const bubble = it.text ? h('div.user-bubble', it.text) : null;
  const edit = button('pencil', t('said.edit'), () => startEdit());
  edit.classList.add('is-edit');
  const mine = media.mine !== false;
  const actions = h('div.msg-actions.user-actions', it.text ? button('copy', t('said.copy'), () => on.copy(it.text)) : null, mine ? edit : null);
  const who = mine ? null : speakerNode(it.speaker, on, media);
  const node = h(`div.user-message${mine ? '' : '.is-other'}`, who, attachmentsNode(it.attachments ?? [], media), bubble, actions);

  // 编辑：气泡换成一个输入框，原话填好、全选；Enter 发、Shift+Enter 换行、Esc 取消
  const startEdit = () => {
    const box = /** @type {HTMLTextAreaElement} */ (h('textarea.edit-input', { rows: 1, spellcheck: 'false' }));
    box.value = it.text;
    const grow = () => {
      box.style.height = 'auto';
      box.style.height = `${box.scrollHeight}px`;
    };
    // 收起：输入框淡出，再换回气泡（蓝图「动效」）
    const stop = () => leave(editor, () => {
      if (bubble) editor.replaceWith(bubble);
      else editor.remove();
      node.classList.remove('is-editing');
    });
    const go = () => {
      const text = box.value.trim();
      if (!text) return;
      stop();
      on.edit(text);
    };
    const editor = h('div.user-edit', box,
      h('div.edit-bar',
        h('button.edit-cancel', { type: 'button', onclick: stop }, t('said.cancel')),
        h('button.edit-send', { type: 'button', onclick: go }, t('said.send'))));
    box.addEventListener('input', grow);
    box.addEventListener('keydown', (e) => {
      if (e.isComposing || e.keyCode === 229) return;
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        go();
      } else if (e.key === 'Escape') {
        e.preventDefault();
        stop();
      }
    });
    node.classList.add('is-editing');
    if (bubble) bubble.replaceWith(editor);
    else actions.before(editor);
    grow();
    box.focus();
    box.select();
  };
  return node;
}

/**
 * 收尾那一行，后面一排复制、重做。
 * @param {{text: string, tone: string, turn: number}} it
 * @param {Actions} on
 */
export function endNode(it, on) {
  const redo = button('rotate-cw', t('said.redo'), () => on.redo());
  redo.classList.add('is-redo');
  // 打断的那一轮，后台还有在跑的：接一句，点了打开后台任务（蓝图「后台任务」第 6 条）
  const jobs = it.jobs ? h('button.turn-end-jobs', { type: 'button', onclick: () => on.openJobs() }, t('interrupted_jobs', { count: it.jobs })) : null;
  return h(`div.turn-end.tone-${it.tone}`,
    h('span.turn-end-text', it.text, jobs),
    h('span.msg-actions.turn-actions', button('copy', t('said.copy'), () => on.copyTurn(it.turn)), redo));
}

/**
 * 附件一排（蓝图「附件」第 6 条）：图片照宽高比先占好地方，最宽最高 `layout.json` 的 `user_media`，点开是灯箱；视频画第一帧、
 * 中间一个播放记号，点了原地换成带控制条的播放器；别的文件是一张小卡（图标照种类），点了经桥的 `/blob` 下载，存成原来的名字。
 * 取不出第一帧的视频照文件画。没有附件的是 `null`。
 * @param {Attachment[]} list
 * @param {Media} media
 */
function attachmentsNode(list, media) {
  if (!list.length) return null;
  const session = media.session;
  return h('div.user-attachments', { style: `--user-media: ${res.layout.user_media}px` }, ...list.map((a) => {
    if (a.kind === 'image') {
      const url = session ? blobUrl(session, a.blob, a.media_type) : '';
      return imageCard({ url, name: a.name ?? '', width: a.width ?? undefined, height: a.height ?? undefined, lightbox: media.lightbox });
    }
    const card = fileCard(a, session);
    return /^video\//.test(a.media_type) && session ? videoThumb(a, session, card) : card;
  }));
}

/** 文件的卡（和框里的一样大，蓝图「附件」第 6 条）：名字、一行小字写媒体类型，左下角扩展名大写（没有扩展名的不写）。 */
function fileCard(a, session) {
  const name = a.name ?? a.blob;
  const dot = name.lastIndexOf('.');
  return h('a.user-file', {
    href: session ? blobUrl(session, a.blob, a.media_type, { download: true, name }) : null,
    title: t('said.download', { name }),
    download: '',
  },
  h('span.user-file-name', name),
  h('span.user-file-sub', a.media_type ?? ''),
  dot > 0 ? h('span.user-file-ext', name.slice(dot + 1).toUpperCase()) : null);
}

/** 视频：第一帧加一个播放记号，点了原地换成带控制条的播放器；取不出第一帧的换成文件的小卡。 */
function videoThumb(a, session, card) {
  const url = blobUrl(session, a.blob, a.media_type);
  const video = /** @type {HTMLVideoElement} */ (h('video', { src: `${url}#t=0.1`, muted: true, preload: 'metadata', playsinline: true }));
  const node = h('div.user-video', { role: 'button', tabindex: '0', title: a.name ?? '' }, video, h('span.user-video-play', icon('play')));
  video.addEventListener('error', () => node.replaceWith(card), { once: true });
  const play = () => {
    const player = h('video.user-video-player', { src: url, controls: true, autoplay: true, playsinline: true });
    node.replaceWith(player);
  };
  node.addEventListener('click', play);
  node.addEventListener('keydown', (e) => {
    if (e.key !== 'Enter' && e.key !== ' ') return;
    e.preventDefault();
    play();
  });
  return node;
}

/**
 * 气泡上面那一行小字：是谁说的。子代理的会话里派它的会话发来的，写「来自「标题」」、后面一个小箭头，点了回到那个会话（蓝图「不是你
 * 说的话」）；标题拿不到的写「派它的会话」。别的会话发来的写「从会话 短编号「标题」收到消息」，会话表里有它的才带箭头、能点开；
 * 没标题的不写「」（2026-10-01 项目主人定，照终端）。
 */
function speakerNode(speaker, on, media) {
  const id = speaker?.id;
  if ((speaker?.kind !== 'parent' && speaker?.kind !== 'session') || !id) return h('div.user-speaker', speaker?.name ?? '');
  const title = media.titleOf?.(id) ?? null;
  if (speaker.kind === 'session' && !title) return h('div.user-speaker', speaker.name);
  const text = speaker.kind === 'parent'
    ? (title ? t('said.from', { title }) : speaker.name)
    : t('said.from_session', { id: shortSession(id), title });
  return h('button.user-speaker.is-link', { type: 'button', title: title ?? speaker.name, onclick: () => on.openSession(id) },
    h('span.user-speaker-text', text), icon('arrow-up-right'));
}

/** 一个图标按钮：名字写在 `title`、`aria-label` 里。 */
function button(name, label, onclick) {
  return h('button.msg-action', { type: 'button', title: label, 'aria-label': label, onclick }, icon(name));
}
