// @ts-check
//! 连不上桥时写什么（蓝图 `web.md`「连核心」第 9 条）：为什么、怎么办。为什么照桥的 `/key` 分：口令用不了（桥每次重启都换
//! 口令）、地址里没带口令、桥没在跑；口令对、桥连不上核心的照桥的原话。纯函数。

import { t } from '../util/res.js';

/**
 * @param {'bad'|'none'|'down'|'core'} kind
 * @param {string} [message] 桥的原话（`core` 时）
 * @returns {{title: string, why: string, how: string}}
 */
export function offline(kind, message = '') {
  const title = t('boot.title');
  if (kind === 'core') return { title, why: message, how: t('boot.core_how') };
  if (kind === 'down') return { title, why: t('boot.down_why'), how: t('boot.down_how') };
  return { title, why: t(kind === 'none' ? 'boot.none_why' : 'boot.bad_why'), how: t('boot.key_how') };
}
