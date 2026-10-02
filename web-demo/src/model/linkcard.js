// @ts-check
//! 链接卡片照 `link.preview` 的回应画什么（蓝图 `web.md`「链接卡片」第 5 条）：视频（`kind` 是 `video`）的配图上叠播放记号、右下角写
//! 时长；站名后面接作者（UP 主、频道名）。网站专门的取法在核心，这里只照回应的几格。纯函数。

/**
 * 时长：秒数写成 `3:33`、`1:02:03`（不足一秒的舍掉）；没有的、不对的是 `null`。
 * @param {unknown} secs
 */
export function duration(secs) {
  if (typeof secs !== 'number' || !Number.isFinite(secs) || secs < 0) return null;
  const s = Math.floor(secs);
  const pad = (n) => String(n).padStart(2, '0');
  const h = Math.floor(s / 3600);
  const m = Math.floor(s / 60) % 60;
  return h ? `${h}:${pad(m)}:${pad(s % 60)}` : `${m}:${pad(s % 60)}`;
}

/**
 * 一张卡片的几样：叠不叠播放记号、时长、站名那一行（站名 · 作者）。
 * @param {{kind?: string, site?: string|null, author?: string|null, duration?: number, image?: unknown}} card
 * @returns {{video: boolean, duration: string|null, site: string}}
 */
export function cardView(card) {
  const video = card.kind === 'video' && !!card.image;
  const site = [card.site, card.author].filter((x) => typeof x === 'string' && x.trim()).join(' · ');
  return { video, duration: video ? duration(card.duration) : null, site };
}
