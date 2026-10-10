// @ts-check
//! 输入框下面那一行要写的字（蓝图 `web.md`「框下面那一行的数」，照 TUI 演示的 `ui/footer.rs`、`core/push.rs`）：
//! 左边权限级别和模型，右边速度 · 上下文 · 累计；还是 0 的格子不写；放不下时右边先丢速度、再丢累计。

import { res, t } from '../util/res.js';
import { short, hitRate, percentTenths } from './format.js';
import { levelOf } from './entries.js';

const input = (u) => u.uncached + u.cache_read + u.cache_write;

/**
 * 照会话状态写（蓝图 `web.md`「照条目画」第 3 条；核心 9-8 补上的 `view.status`）：级别照 `permission`，模型照 `model`，速度照 `speed`
 * （输出 ÷ 首字到说完），上下文照 `context.used`（窗口照 `context.window`，没有的照订阅回应的限额），累计照 `usage`（全部请求，命中率照
 * 主请求）；还是 0 的格子不写。
 * @param {any} status @param {{window?: number}} [limits] 订阅回应里的限额
 * @returns {{left: {level: string, label: string, model: string|null, endpoint: string|null}, right: {key: string, text: string}[]}}
 */
export function statusFooter(status, limits = {}) {
  const level = status?.permission ? levelOf(status.permission) : 'workspace';
  const right = [];
  const speed = status?.speed;
  if (speed && speed.output > 0 && speed.ms > 0) right.push({ key: 'speed', text: t('speed', { rate: Math.round((speed.output * 1000) / speed.ms) }) });
  const used = status?.context?.used ?? 0;
  const window = status?.context?.window ?? limits.window;
  if (used > 0) right.push({ key: 'context', text: window ? t('context', { used: short(used), window: short(window), percent: percentTenths(used, window) }) : short(used) });
  const all = status?.usage?.usage;
  const main = status?.usage?.main;
  if (all && input(all) + all.output > 0) {
    right.push({ key: 'total', text: t('total', { tokens: short(input(all) + all.output), percent: hitRate(main?.cache_read ?? 0, main ? input(main) : 0) }) });
  }
  return { left: { level, label: levelLabel(level), model: status?.model?.model ?? null, endpoint: status?.model?.endpoint ?? null }, right };
}

/** 左边的级别：图标加名字，`▣ 工作区`（图标照 TUI 的 `layout.json`）。 */
export const levelLabel = (level) => `${res.layout.level_icons[level]}${res.text.levels[level]}`;

/** 点一下换到下一级：照 `layout.json` 的 `level_cycle`，工作区 → 开放权限 → 只读 → 工作区（照 TUI 的 Tab）。 */
export function nextLevel(level) {
  const cycle = res.layout.level_cycle;
  return cycle[(cycle.indexOf(level) + 1) % cycle.length];
}

/**
 * 换到这一级发给核心的参数（`session.set_permission_level`，蓝图 `protocol.md`）：只读只开只读开关，常用的那一级
 * 不动；工作区、开放权限写级别，并关掉只读。
 * @param {string} level 换到的那一级
 */
export function levelParams(level) {
  return level === 'read_only' ? { read_only: true } : { level, read_only: false };
}

/** 丢格子的先后：先速度，再累计，上下文留到最后。 */
const DROP = ['speed', 'total', 'context'];

/**
 * 右边放得下的几格：照 `DROP` 的先后一格格丢，直到 `measure` 量出来不超过 `room`。
 * @param {{key: string, text: string}[]} parts
 * @param {number} room 放得下多宽
 * @param {(parts: {key: string, text: string}[]) => number} measure 量这几格连起来多宽
 */
export function fit(parts, room, measure) {
  let kept = parts;
  for (const key of DROP) {
    if (kept.length === 0 || measure(kept) <= room) return kept;
    kept = kept.filter((p) => p.key !== key);
  }
  return [];
}
