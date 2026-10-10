// @ts-check
//! 供应商的图标（蓝图 `web.md`「供应商的图标」；2026-10-11 项目主人看过两套的对比定：有彩色的用 lobe-icons 的彩色版，其余用 models.dev 的
//! 单色）：核心在 `provider.catalog`、`model.list` 的每一家给 `logo: {svg, tint}`，图标由核心拉好存在本地，网页不连外网。
//!
//! - 单色的（`tint`）照字色画：CSS 遮罩铺 `currentColor`，深色、浅色、跟着人格的配色都跟着走；
//! - 彩色的照原样画：`<img>` 的 data 地址；
//! - 都不把 SVG 插进页面（图里带脚本也跑不了）；没有图的画名称的第一个字；
//! - 老核心没有 `logo` 这一格：不画图标位，照原来的样子（`hasLogos`）。

import { h } from './dom.js';

/** @typedef {{svg: string, tint: boolean}|null|undefined} Logo 核心给的一家的图标 */

/** 核心给不给图标：有一项带 `logo` 这一格（哪怕是 `null`）就是给。 @param {any[]|null|undefined} items */
export const hasLogos = (items) => (items ?? []).some((x) => x && typeof x === 'object' && 'logo' in x);

/** SVG 原文写成 data 地址。 @param {string} svg */
export const svgUrl = (svg) => `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;

/**
 * 画成哪一种：单色（`tint`，照字色）、彩色（`color`，照原样）、没有图的写名称的第一个字（`initial`）。
 * @param {Logo} logo @param {string} name
 * @returns {{kind: 'tint'|'color', url: string}|{kind: 'initial', text: string}}
 */
export function logoPlan(logo, name) {
  if (logo && typeof logo.svg === 'string' && logo.svg) return { kind: logo.tint ? 'tint' : 'color', url: svgUrl(logo.svg) };
  return { kind: 'initial', text: ([...String(name ?? '').trim()][0] ?? '').toUpperCase() };
}

/** 一家的图标。 @param {Logo} logo @param {string} name */
export function logoEl(logo, name) {
  const plan = logoPlan(logo, name);
  if (plan.kind === 'color') return h('img.prov-logo', { src: plan.url, alt: '', 'aria-hidden': 'true' });
  if (plan.kind === 'tint') {
    const el = h('span.prov-logo.is-tint', { 'aria-hidden': 'true' });
    el.style.setProperty('--logo', `url("${plan.url}")`);
    return el;
  }
  return h('span.prov-logo.is-initial', { 'aria-hidden': 'true' }, plan.text);
}
