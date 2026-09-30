// @ts-check
//! 输入框下面那一行要写的字（蓝图 `web.md`「框下面那一行的数」，照 TUI 演示的 `ui/footer.rs`、`core/push.rs`）：
//! 左边权限级别和模型，右边速度 · 上下文 · 累计；还是 0 的格子不写；放不下时右边先丢速度、再丢累计。

import { res, t } from '../util/res.js';
import { short, hitRate, percentTenths } from './format.js';
import { levelOf } from './transcript.js';

const input = (u) => u.uncached + u.cache_read + u.cache_write;

/**
 * @param {any[]} events 这个会话的持久事件
 * @param {{window?: number}} limits `subscribe` 回应里的限额（蓝图 `protocol.md`）
 */
export function footer(events, limits) {
  let level = 'workspace';
  let model = null;
  let endpoint = null;
  let speed = null;
  let context = 0;
  const total = { input: 0, output: 0, hit: 0 };
  for (const e of events) {
    const b = e.body;
    if ((e.kind === 'session.created' || e.kind === 'session.policy_changed') && b.permission) level = levelOf(b.permission);
    // 清空了：上下文清零，下一次请求再照实际的写（蓝图 `web.md`「压缩、清空」）
    if (e.kind === 'context.compacted' && b.trigger === 'clear') context = 0;
    if (e.kind !== 'model.called') continue;
    if (b.model) ({ model, endpoint } = b);
    if (!b.usage) continue;
    total.input += input(b.usage);
    total.output += b.usage.output;
    total.hit += b.usage.cache_read;
    // 上下文照主请求算：压缩的摘要请求看的是另一份东西
    if (!b.compaction) context = input(b.usage) + b.usage.output;
    // 出字的速度：输出 ÷ 从第一个字到说完的时间
    if (b.duration_ms != null && b.first_token_ms != null && b.usage.output > 0 && b.duration_ms > b.first_token_ms) {
      speed = (b.usage.output * 1000) / (b.duration_ms - b.first_token_ms);
    }
  }
  const right = [];
  if (speed != null) right.push({ key: 'speed', text: t('speed', { rate: Math.round(speed) }) });
  if (context > 0) {
    const text = limits.window
      ? t('context', { used: short(context), window: short(limits.window), percent: percentTenths(context, limits.window) })
      : short(context);
    right.push({ key: 'context', text });
  }
  if (total.input + total.output > 0) {
    right.push({ key: 'total', text: t('total', { tokens: short(total.input + total.output), percent: hitRate(total.hit, total.input) }) });
  }
  return { left: { level, label: levelLabel(level), model, endpoint }, right };
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
