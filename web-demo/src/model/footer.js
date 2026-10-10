// @ts-check
//! 输入框下面那一行要写的字（蓝图 `web.md`「框下面那一行的数」，照 TUI 演示的 `ui/footer.rs`、`core/push.rs`）：
//! 左边权限级别和模型，右边速度 · 上下文 · 累计；还是 0 的格子不写；放不下时右边先丢速度、再丢累计。

import { res, t } from '../util/res.js';
import { short, hitRate, percentTenths } from './format.js';
import { levelOf } from './entries.js';

const input = (u) => u.uncached + u.cache_read + u.cache_write;

/**
 * @param {any[]} events 这个会话的持久事件
 * @param {{window?: number}} limits `subscribe` 回应里的限额（蓝图 `protocol.md`）
 * @param {Map<number, {before: number, after: number}>} [stats] 看着压好的那几次压缩的前后用量，照落了盘的那一条的序号（`core/store.js`）
 * @param {{endpoint?: string, model?: string, ref?: string}|null} [next] 会话接下来请求的模型（`subscribe` 回应、`model.changed`，
 *   核心施工 8-10）：有 `model` 的照它写；轮换的池只有 `ref`、没有的照最近一次 `model.called`
 * @param {import('../core/store.js').Base|null} [base] 订阅回应里「这一刻的」（核心 9-6 上）：累计、权限级别照它起头，之后只加序号比
 *   `upto` 大的（往上翻只读进来一部分日志也对）；累计 token 照 `usage.usage`（全部请求），命中率照 `usage.main`（只算主请求，9-6 上补）。
 *   没有的照读进来的日志从头算
 */
export function footer(events, limits, stats = new Map(), next = null, base = null) {
  const upto = base?.upto ?? 0;
  const all = base?.usage?.usage ?? null;
  const main = base?.usage?.main ?? null;
  let level = base?.permission ? levelOf(base.permission) : 'workspace';
  let model = null;
  let endpoint = null;
  let speed = null;
  let context = 0;
  const total = all ? { input: input(all), output: all.output, hit: 0, mainInput: 0 } : { input: 0, output: 0, hit: 0, mainInput: 0 };
  if (main) Object.assign(total, { mainInput: input(main), hit: main.cache_read });
  for (const e of events) {
    const b = e.body;
    const after = e.seq > upto;
    if ((e.kind === 'session.created' || e.kind === 'session.policy_changed') && b.permission && (!base?.permission || after)) level = levelOf(b.permission);
    // 清空了：上下文清零，下一次请求再照实际的写（蓝图 `web.md`「压缩、清空」）
    if (e.kind === 'context.compacted' && b.trigger === 'clear') context = 0;
    // 压好了：换成压完的用量（看着压好的那一次核心估的 `after`）；读回来的不知道，先不写，下一次主请求再照实际的写
    else if (e.kind === 'context.compacted') context = stats.get(e.seq)?.after ?? 0;
    if (e.kind !== 'model.called') continue;
    if (b.model) ({ model, endpoint } = b);
    if (!b.usage) continue;
    if (!all || after) {
      total.input += input(b.usage);
      total.output += b.usage.output;
    }
    // 回顾这类辅助请求（带 `purpose`）算进累计，不改上下文、命中率、速度（蓝图「回顾」第 5 条）：单独发、不命中缓存
    if (b.purpose) continue;
    if (!main || after) {
      total.mainInput += input(b.usage);
      total.hit += b.usage.cache_read;
    }
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
    right.push({ key: 'total', text: t('total', { tokens: short(total.input + total.output), percent: hitRate(total.hit, total.mainInput) }) });
  }
  if (next?.model) ({ model, endpoint = null } = next);
  return { left: { level, label: levelLabel(level), model, endpoint }, right };
}

/**
 * 照会话状态写（蓝图 `web.md`「照条目画」第 3 条；核心 9-8 补上的 `view.status`）：级别照 `permission`，模型照 `model`，速度照 `speed`
 * （输出 ÷ 首字到说完），上下文照 `context.used`（窗口照 `context.window`，没有的照订阅回应的限额），累计照 `usage`（全部请求，命中率照
 * 主请求）。形状同 `footer`。
 * @param {any} status @param {{window?: number}} [limits]
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
