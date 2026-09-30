// @ts-check
//! 界面语言（蓝图 `web.md`「界面语言」，照 `tui.md`「界面语言」）：一张表（`resources/languages.json`）、内核的设置项
//! `language`（`auto` 或表里的一种）、用哪一种、`/language` 浮层里的几行。纯函数，`boot.js` 照它定语言。

import { resolve } from './config.js';

/**
 * @typedef {{code: string, name: string, tag: string, locales: string[]}} Entry 表里的一种：代码（`zh`，和核心资源目录的
 *   `human/<代码>.json` 同一套）、自己的名字、认哪些浏览器语言的前缀、页面的语言标签（`<html lang>`、握手的 `locale`）
 * @typedef {{fallback: string, auto_summary?: string, languages: Entry[]}} Table `auto_summary`：跟着浏览器时时间线收起那一行用哪一种
 * @typedef {Entry & {fallback: string, summary: string}} Language 定下来的一种，带着缺字时退回哪一种、时间线收起那一行用哪一种
 *   （手动定了语言的是它自己，跟着浏览器的是表里的 `auto_summary`，英文；2026-10-01 项目主人定）
 */

/** 跟着浏览器的那个值 */
export const AUTO = 'auto';

/**
 * 设置项的规格：`auto` 加表里的每一种，出厂 `auto`，改了重新载入页面（界面的字在内核起来时装）。
 * @param {Table} table
 * @returns {import('./config.js').Spec}
 */
export function languageSpec(table) {
  return { type: 'choice', choices: [AUTO, ...table.languages.map((l) => l.code)], default: AUTO, applies: 'reload' };
}

/**
 * 设置项的最终值：出厂 → 发行版 → 个人，写错的这一项用下面一层的（`config.js` 的 `resolve`）。
 * @param {Table} table
 * @param {Record<string, any>} distro `distro.json` 的 `kernel`
 * @param {Record<string, any>} user 个人那一层 `kernel` 的 `config`
 */
export function settingOf(table, distro, user) {
  const layers = [{ name: 'distro', values: distro }, { name: 'user', values: user }];
  return resolve({ language: languageSpec(table) }, layers).values.language;
}

/**
 * 用哪一种：设置项写了哪种用哪种；`auto` 照浏览器的语言（`navigator.languages`）依次认表里的前缀，一个都认不出的用退回的
 * 那一种。
 * @param {string} setting
 * @param {readonly string[]} browser
 * @param {Table} table
 * @returns {Language}
 */
export function pick(setting, browser, table) {
  const byCode = (code) => table.languages.find((l) => l.code === code);
  const matched = (tag) => table.languages.find((l) => l.locales.some((p) => tag.toLowerCase().startsWith(p.toLowerCase())));
  const entry = (setting !== AUTO && byCode(setting)) || browser.map(matched).find(Boolean) || byCode(table.fallback) || table.languages[0];
  const summary = setting === AUTO ? table.auto_summary ?? entry.code : entry.code;
  return { ...entry, fallback: table.fallback, summary };
}

/**
 * `/language` 浮层里的几行（蓝图 `web.md`「界面语言」第 3 条）：先是跟着浏览器（`auto`，名字写它现在认成的那一种），再是表里的
 * 每一种（名字是它自己的）；`current` 是设置项现在写的那一行。
 * @param {string} setting 设置项现在的值
 * @param {readonly string[]} browser 浏览器的语言
 * @param {Table} table
 * @returns {{items: {value: string, name: string, auto: boolean}[], current: number}}
 */
export function options(setting, browser, table) {
  const items = [
    { value: AUTO, name: pick(AUTO, browser, table).name, auto: true },
    ...table.languages.map((l) => ({ value: l.code, name: l.name, auto: false })),
  ];
  return { items, current: Math.max(0, items.findIndex((x) => x.value === setting)) };
}
