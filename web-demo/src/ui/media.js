// @ts-check
//! 图片、视频、音频卡片（蓝图 `web.md`「图片」「音视频、图片卡片」）：照旧版 `app.js:4867-4891`（视频）、`6455-6535`（图片）、
//! `shared.js:97-290`（音频播放器）。地址由调用的一方给（本机的经桥的 `/file`、`/blob`，网上的照原样）。
//!
//! - 图片：最宽最高 250px，按图的宽高先占好地方；点开是灯箱；读不出来的写一句，本机的再写按哪个路径找的。
//! - 视频：原生控制条，右上角一个全页播放的按钮（悬停才露），下面一行说明。
//! - 音频：旧版的自制播放器：封面块、名字、时长；播放、进度条（点、拖、左右键 5 秒）、静音、音量、下载。

import { h, icon, replace } from './dom.js';
import { res, t } from '../util/res.js';
import { openExternal } from '../core/host.js';

/**
 * 一张图（`.conversation-media`）。知道宽高的先按比例占好地方，图来了不跳。
 * @param {{url: string, name?: string, width?: number, height?: number, workspace?: () => void, lightbox?: () => any, tried?: string}} what
 *   `lightbox` 交回现在的灯箱（软件包 lightbox 的服务；没装是 `undefined`）；`tried` 本机的图按哪个路径找（读不出来时写出来）
 */
export function imageCard(what) {
  const img = /** @type {HTMLImageElement} */ (h('img', { src: what.url, alt: what.name ?? '', loading: 'lazy', decoding: 'async' }));
  const fallback = h('div.conversation-media-fallback', { hidden: true }, icon('circle-alert'), h('span', t('media.image_failed')),
    what.tried ? h('span.conversation-media-tried', what.tried) : null);
  const ratio = what.width && what.height && what.width / what.height;
  const visual = h(`div.conversation-media-visual${ratio && ratio > 0.05 && ratio < 20 ? '.has-aspect' : ''}`,
    { role: 'button', tabindex: '0', title: what.name ?? '', style: ratio ? `aspect-ratio: ${what.width} / ${what.height}` : null }, img, fallback);
  img.addEventListener('error', () => {
    img.remove();
    fallback.hidden = false;
    visual.removeAttribute('role');
  }, { once: true });
  // 点开：有灯箱（软件包 lightbox）在灯箱里看，没有的经宿主在外面开
  const open = () => {
    if (!img.isConnected) return;
    const box = what.lightbox?.();
    if (box) box.open({ url: what.url, name: what.name, workspace: what.workspace, vector: /\.svg$/i.test(what.name ?? '') });
    else openExternal(what.url);
  };
  visual.addEventListener('click', open);
  visual.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      open();
    }
  });
  return h('figure.conversation-media', visual);
}

/**
 * 一段视频（`.video-card`）：原生控制条；右上角的按钮铺满整页再点收回。
 * @param {{url: string, name: string}} what
 */
export function videoCard(what) {
  const video = h('video', { src: what.url, controls: true, preload: 'metadata', playsinline: true });
  const shell = h('div.video-shell', video);
  const full = h('button.vfs-btn', { type: 'button', title: t('media.fullscreen'), 'aria-label': t('media.fullscreen') }, icon('maximize-2'));
  full.addEventListener('click', () => {
    const on = shell.classList.toggle('webfs');
    replace(full, icon(on ? 'minimize-2' : 'maximize-2'));
  });
  shell.append(full);
  return h('div.video-card', shell, h('div.video-caption', { title: what.name }, what.name));
}

/**
 * 一段音频：旧版的自制播放器（`shared.js:97-290`）。
 * @param {{url: string, name: string, download?: string}} what `download` 是下载的地址，没有的不画下载
 */
export function audioCard(what) {
  const audio = /** @type {HTMLAudioElement} */ (h('audio', { src: what.url, preload: 'metadata' }));
  const meta = h('div.media-audio-meta', '--:--');
  const play = h('button.media-audio-play', { type: 'button' });
  const now = h('span.media-audio-time', '0:00');
  const total = h('span.media-audio-time', '--:--');
  const fill = h('div.media-audio-track-fill');
  const track = h('div.media-audio-track.is-static', { role: 'slider', tabindex: '0', 'aria-label': t('media.progress'), 'aria-valuemin': '0', 'aria-valuenow': '0' }, fill);
  const mute = h('button.media-audio-icon-btn', { type: 'button' });
  const volume = /** @type {HTMLInputElement} */ (h('input.media-audio-volume', { type: 'range', min: '0', max: '1', step: '0.05', value: '1', title: t('media.volume'), 'aria-label': t('media.volume') }));
  const download = what.download
    ? h('a.media-audio-icon-btn', { href: what.download, download: what.name, title: t('media.download'), 'aria-label': t('media.download') }, icon('download'))
    : null;
  const card = h('div.media-audio-card',
    h('div.media-audio-head', h('div.media-audio-cover', icon('music')), h('div.media-audio-info', h('div.media-audio-name', { title: what.name }, what.name), meta)),
    h('div.media-audio-controls', play, now, track, total, mute, volume, download),
    audio);

  const label = (el, name, text) => {
    replace(el, icon(name));
    el.title = text;
    el.setAttribute('aria-label', text);
  };
  const syncPlay = () => {
    const playing = !audio.paused && !audio.ended;
    label(play, playing ? 'pause' : 'play', t(playing ? 'media.pause' : 'media.play'));
    card.classList.toggle('is-playing', playing);
  };
  const syncDuration = () => {
    const d = audio.duration;
    const known = Number.isFinite(d) && d > 0;
    track.classList.toggle('is-static', !known);
    if (!known) return;
    total.textContent = clock(d);
    meta.textContent = clock(d);
    track.setAttribute('aria-valuemax', String(Math.floor(d)));
  };
  const syncProgress = () => {
    const d = audio.duration;
    const ratio = Number.isFinite(d) && d > 0 ? audio.currentTime / d : 0;
    fill.style.width = `${(Math.min(Math.max(ratio, 0), 1) * 100).toFixed(2)}%`;
    now.textContent = clock(audio.currentTime);
    track.setAttribute('aria-valuenow', String(Math.floor(audio.currentTime || 0)));
  };
  const syncVolume = () => {
    const muted = audio.muted || audio.volume === 0;
    label(mute, muted ? 'volume-x' : 'volume-2', t(muted ? 'media.unmute' : 'media.mute'));
    volume.value = String(audio.muted ? 0 : audio.volume);
  };
  play.addEventListener('click', () => {
    if (audio.paused || audio.ended) audio.play().catch((err) => console.error(`放不了 ${what.name}：${err.message}`));
    else audio.pause();
  });
  mute.addEventListener('click', () => { audio.muted = !audio.muted; });
  volume.addEventListener('input', () => {
    audio.volume = Number(volume.value);
    audio.muted = audio.volume === 0;
  });
  for (const e of ['play', 'pause', 'ended']) audio.addEventListener(e, syncPlay);
  audio.addEventListener('loadedmetadata', () => { syncDuration(); syncProgress(); });
  audio.addEventListener('durationchange', syncDuration);
  audio.addEventListener('timeupdate', syncProgress);
  audio.addEventListener('volumechange', syncVolume);
  audio.addEventListener('error', () => {
    meta.textContent = t('media.audio_failed');
    card.classList.add('is-error');
  });

  // 进度条：点、拖到哪放到哪；左右键跳几秒照 `cards.json` 的 `seek_seconds`，空格、回车播放暂停
  const seek = (x) => {
    const d = audio.duration;
    const box = track.getBoundingClientRect();
    if (!Number.isFinite(d) || d <= 0 || box.width <= 0) return;
    audio.currentTime = Math.min(Math.max((x - box.left) / box.width, 0), 1) * d;
    syncProgress();
  };
  let dragging = false;
  track.addEventListener('pointerdown', (e) => {
    if (!Number.isFinite(audio.duration) || audio.duration <= 0) return;
    dragging = true;
    track.classList.add('is-scrubbing');
    track.setPointerCapture(e.pointerId);
    seek(e.clientX);
    e.preventDefault();
  });
  track.addEventListener('pointermove', (e) => { if (dragging) seek(e.clientX); });
  const stop = (/** @type {PointerEvent} */ e) => {
    if (!dragging) return;
    dragging = false;
    track.classList.remove('is-scrubbing');
    if (track.hasPointerCapture(e.pointerId)) track.releasePointerCapture(e.pointerId);
  };
  track.addEventListener('pointerup', stop);
  track.addEventListener('pointercancel', stop);
  track.addEventListener('keydown', (e) => {
    const d = audio.duration;
    if (e.key === ' ' || e.key === 'Enter') play.click();
    else if (!Number.isFinite(d) || d <= 0) return;
    else if (e.key === 'ArrowRight' || e.key === 'ArrowUp') audio.currentTime = Math.min(audio.currentTime + res.cards.seek_seconds, d);
    else if (e.key === 'ArrowLeft' || e.key === 'ArrowDown') audio.currentTime = Math.max(audio.currentTime - res.cards.seek_seconds, 0);
    else return;
    e.preventDefault();
  });
  syncPlay();
  syncVolume();
  return card;
}

/** 播放器上的时间：`m:ss`，一小时以上 `h:mm:ss`。 */
function clock(secs) {
  const s = Math.max(0, Math.floor(secs || 0));
  const pad = (n) => String(n).padStart(2, '0');
  const hours = Math.floor(s / 3600);
  return hours ? `${hours}:${pad(Math.floor(s / 60) % 60)}:${pad(s % 60)}` : `${Math.floor(s / 60)}:${pad(s % 60)}`;
}
