// @ts-check
//! 时间线上的字（蓝图 `web.md`「时间线」，照旧版网页 `app.js:6962-7051`、`8334-8771`）：一步那一行、思考收着时的那一小段
//! 和在想时的窗口、命令写在下面的几行、点开的细节。收起那一行照 TUI（`tui.md`「时间线」第 17 条，TUI 演示的
//! `ui/timeline/summary.rs`）。纯函数，界面照它画。
//!
//! 给人看的字（显示名、对象是哪个参数、结果那一句）照 `res.human`（桥的 `web.human`，和 TUI、`miyu ask` 同一份）；
//! 图标是 `resources/lucide.json` 里的名字，照 `timeline.json` 的 `icons`。

import { res, t } from '../util/res.js';
import { clock, tilde, toolDuration } from './format.js';
import { fromArgs } from './diff.js';

/** 留言发给父会话时 `to` 写的（`tools/message_agent.md`） */
const PARENT = 'parent';

/** 一件工具算哪一类：`command`、`edit`、`agent`、`message`；没登记的是 `null`（`timeline.json` 的 `kinds`，收起那一行照它数）。 */
export const kindOf = (name) => res.timeline.kinds[name] ?? null;

/**
 * 出错了：图标换成 `circle-alert`、整行变红。`error` 和 `denied`（被拒：只读时的写入、要确认却没人能确认的）都算，被拒的写入不能
 * 看着像写成了（照 `tui.md`「时间线」第 12 条）；打断的不算。
 */
export const failed = (step) => step.kind === 'tool' && (step.status === 'error' || step.status === 'denied');

/**
 * @typedef {{since: number, format: 'secs'|'tenths'|'job'}} Timer 走表：从 `since` 起，`secs` 整秒 `12s`、`tenths`
 *   一位小数 `1.2 s`、`job` 读秒 `3m 05s`
 * @typedef {{icon: string, name: string, subject: string|null, mono: boolean, said: string|null, took: string|null,
 *   timer: Timer|null, failed: boolean, diff: {added: number, removed: number}|null}} Row `diff`：编辑、写入这一步加减的行数
 */

/**
 * 一步那一行：图标、名字、对象（别的工具的对象用等宽字，执行命令的是短标题）、结果那一句、用时。
 * @param {import('./timeline.js').Step} step
 * @param {string|null} home 家目录：里面的路径写成 `~/…`
 * @returns {Row}
 */
export function row(step, home) {
  const base = { subject: null, mono: false, said: null, took: null, timer: null, failed: false, diff: null };
  if (step.kind === 'thought') {
    if (step.state === 'thinking') {
      return { ...base, icon: 'atom', name: t('timeline.thinking'), timer: step.start != null ? { since: step.start, format: 'secs' } : null };
    }
    // 读回来的历史不知道想了多久：不写用时（蓝图「时间线的数」第 4 条；旧版也不写）
    const took = step.start != null && step.end != null ? `${((step.end - step.start) / 1000).toFixed(1)}s` : null;
    return { ...base, icon: 'atom', name: t('timeline.thought'), took };
  }
  const kind = kindOf(step.name);
  if (step.state === 'preparing') {
    const which = kind === 'command' ? 'command' : kind === 'edit' || step.name === 'trash' ? 'edit' : 'tool';
    return { ...base, icon: 'loader-circle', name: t(`timeline.prepare.${which}`), timer: step.start != null ? { since: step.start, format: 'tenths' } : null };
  }
  const bad = failed(step);
  const icon = bad ? 'circle-alert' : res.timeline.icons[step.name] ?? res.timeline.icon_default;
  const face = res.human?.tools?.[step.name];
  const name = face?.name ?? step.name;
  if (kind === 'command') {
    // 执行命令：对象是短标题，命令本身写在下面；用时照结果的 duration_ms，在跑的走表
    const took = step.state === 'done' && step.duration != null ? toolDuration(step.duration) : null;
    const timer = step.state === 'running' && step.start != null ? { since: step.start, format: /** @type {const} */ ('job') } : null;
    return { ...base, icon, name, subject: arg(step, 'description'), took, timer, failed: bad };
  }
  if (kind === 'agent') {
    // 派子代理：「派子代理 · 编号 · 标题」（编号照结果里的 `job.started`，还没派出去的只写标题），不写结果那一句；点开是提示词
    // （2026-09-30 项目主人定）
    const title = arg(step, 'description') ?? '';
    const subject = step.job ? t('timeline.agent_subject', { job: step.job, title }) : t('timeline.agent_pending', { title });
    return { ...base, icon, name, subject, failed: bad };
  }
  if (kind === 'message') {
    // 留言：「留言 · j2」（发给父会话的写「父会话」），送到了不写结果那一句（和对象重了），没送到的照写；收着时后面接留言
    // 开头的预览（`messagePeek`），点开是发给谁、完整的消息（2026-10-01 项目主人定）
    const to = arg(step, 'to');
    const subject = to ? t('timeline.message_subject', { to: to === PARENT ? t('timeline.parent') : to }) : null;
    return { ...base, icon, name, subject, said: step.status === 'ok' ? null : say(step.said), failed: bad };
  }
  const subject = face?.subject ? arg(step, face.subject) : null;
  return { ...base, icon, name, subject: subject ? tilde(subject, home) : null, mono: true, said: say(step.said), failed: bad, diff: counts(step) };
}

/**
 * 思考收着时接在那一行后面的一小段：最后 `peek_chars` 个字，空白压成一个空格（照旧版 `app.js:6563`）。截掉了前面的
 * 打头写 `…`；截在一个英文词中间的，那半个词不要，不然开头是 `hat to be` 这样的半截。
 */
export function peek(step) {
  const text = step.kind === 'thought' ? step.text.replace(/\s+/g, ' ').trim() : '';
  const room = res.timeline.peek_chars;
  if (text.length <= room) return text;
  const from = text.length - room + 1;
  let tail = text.slice(from);
  if (/\w/.test(text[from - 1]) && /^\w/.test(tail)) tail = tail.replace(/^\w+\s*/, '');
  return `…${tail}`;
}

/**
 * 没改成的编辑（有了结果、不是成了：出错、被拒、打断）：不算 edit、不算加减的行数，算成用过一件工具（照 `tui.md`「时间线」第 17 条）。
 * 还在跑的照参数算。
 */
function unchanged(step) {
  return step.kind === 'tool' && kindOf(step.name) === 'edit' && step.status != null && step.status !== 'ok';
}

/**
 * 编辑、写入这一步加减的行数（照 `tui.md`「差异」第 5 条，和收起那一行同一份，照参数算）：加减都是 0 的、没改成的（出错、被拒、
 * 打断）是 `null`，免得看着像改了；在跑的照参数写。
 */
function counts(step) {
  if (kindOf(step.name) !== 'edit' || unchanged(step)) return null;
  const d = fromArgs(step.parsed);
  return d && d.added + d.removed > 0 ? { added: d.added, removed: d.removed } : null;
}

/**
 * 留言收着时接在那一行后面的预览：留言开头 `peek_chars` 个字，空白压成一个空格，截了的末尾写 `…`（思考露尾巴，留言露开头）。
 * 不是留言的是空的。
 */
export function messagePeek(step) {
  if (step.kind !== 'tool' || kindOf(step.name) !== 'message') return '';
  const text = (arg(step, 'message') ?? '').replace(/\s+/g, ' ').trim();
  const room = res.timeline.peek_chars;
  return text.length <= room ? text : `${text.slice(0, room - 1)}…`;
}

/** 在想、收着的时候那一行下面滚着显示的：最后 `thinking_rows` 行，首尾的空行不算。 */
export function thinkingTail(step) {
  const text = step.kind === 'thought' ? step.text.trim() : '';
  return text.split('\n').slice(-res.timeline.thinking_rows).join('\n');
}

/**
 * 执行命令写在那一行下面的命令本身：最多 `command_rows` 行；放不下时让出最后一行写 `⋮`（照旧版 `app.js:8408-8525`）。
 * 不是命令、命令还没有字的是 `null`。
 * @returns {{lines: string[], more: boolean}|null}
 */
export function commandLines(step) {
  if (step.kind !== 'tool' || kindOf(step.name) !== 'command') return null;
  const command = arg(step, 'command') ?? '';
  if (!command) return null;
  const lines = command.split('\n');
  const rows = res.timeline.command_rows;
  return lines.length > rows ? { lines: lines.slice(0, rows - 1), more: true } : { lines, more: false };
}

/**
 * 点开一步看到的细节，一段一段（照旧版 `app.js:8673-8696`）：「参数」一行一个 `键: 值`，再是「结果」；编辑、写入是
 * 差异卡片，做成了不写结果。没有的段不写。执行命令点开时命令那几行留着（`tui.md`「时间线」第 14 条：命令、空行、
 * 输出），参数里不再写 `command`（那几行写着）、`description`（标题上写着），命令被截了的才把全文写进参数。
 * @returns {({kind: 'text', label: string, text: string}|{kind: 'diff', op: string, created: boolean, path: string, diff: import('./diff.js').Diff})[]}
 */
export function details(step) {
  if (step.kind !== 'tool') return [];
  const output = step.output.trimEnd();
  const result = [
    ...(output ? [{ kind: /** @type {const} */ ('text'), label: t('timeline.result'), text: output }] : []),
    // 结果里的图：接在「结果」下面；没有字的只有图，也写「结果」（蓝图「图片」第 2 条）
    ...(step.images?.length ? [{ kind: /** @type {const} */ ('images'), label: output ? '' : t('timeline.result'), images: step.images }] : []),
  ];
  const diff = kindOf(step.name) === 'edit' ? fromArgs(step.parsed) : null;
  if (diff) {
    const created = !!step.said?.key?.endsWith('/created');
    const card = { kind: /** @type {const} */ ('diff'), op: t(created ? 'timeline.created' : 'timeline.changed'), created, path: arg(step, 'file_path') ?? '', diff };
    return step.status === 'ok' ? [card] : [card, ...result];
  }
  // 派子代理：完整的提示词；出错了的接着结果
  if (kindOf(step.name) === 'agent') {
    const prompt = arg(step, 'prompt');
    return [...(prompt ? [{ kind: /** @type {const} */ ('text'), label: t('timeline.prompt'), text: prompt }] : []), ...(step.status === 'ok' ? [] : result)];
  }
  // 留言：发给谁（编号加子代理的标题，父会话写「父会话」）、完整的消息；没送到的接着结果
  if (kindOf(step.name) === 'message') {
    const to = arg(step, 'to');
    const who = to === PARENT ? t('timeline.parent') : to && step.toTitle ? t('timeline.message_to', { job: to, title: step.toTitle }) : to;
    const message = arg(step, 'message');
    return [
      ...(who ? [{ kind: /** @type {const} */ ('text'), label: t('timeline.to'), text: who }] : []),
      ...(message ? [{ kind: /** @type {const} */ ('text'), label: t('timeline.message'), text: message }] : []),
      ...(step.status === 'ok' ? [] : result),
    ];
  }
  const lines = commandLines(step);
  const shown = lines && !lines.more ? ['command', 'description'] : lines ? ['description'] : [];
  const args = pretty(step, shown);
  return [...(args ? [{ kind: /** @type {const} */ ('text'), label: t('timeline.args'), text: args }] : []), ...result];
}

/**
 * 收起那一行（`tui.md`「时间线」第 17 条）：永远英文，不带箭头，末尾接这一段的用时。一格一格用 ` · ` 接起来；编辑那一格
 * 后面接加减的总行数（`tone` 是 `added`、`removed`）。只有一条命令、它出错了的，整行红（`failed`）。
 * @param {import('./timeline.js').Step[]} steps
 * @param {number} now 还在进行的步算到这一刻
 * @returns {{spans: {text: string, tone: 'base'|'added'|'removed'}[], failed: boolean}}
 */
export function summary(steps, now) {
  const words = res.text.timeline.summary;
  const n = { commands: 0, agents: 0, messages: 0, edits: 0, tools: 0, thoughts: 0, errors: 0 };
  let thinking = 0;
  let known = true;
  for (const step of steps) {
    if (failed(step)) n.errors += 1;
    if (step.kind === 'thought') {
      n.thoughts += 1;
      const took = elapsed(step, now);
      if (took == null) known = false;
      else thinking += took;
      continue;
    }
    const kind = kindOf(step.name);
    if (kind === 'command') n.commands += 1;
    else if (kind === 'agent') n.agents += 1;
    else if (kind === 'message') n.messages += 1;
    else if (kind === 'edit' && !unchanged(step)) n.edits += 1;
    else n.tools += 1;
  }
  const count = (k, forms) => forms[k === 1 ? 0 : 1].replace('{count}', String(k));
  const span = spanOf(steps, now);
  // 用时：四舍五入到整秒，不到一秒的写 1s；一步的时刻都不知道的不写
  const took = span == null ? null : clock(Math.max(1, Math.round(span / 1000)));
  /** @type {[string, boolean][]} 一格一格：字，和这一格要不要接加减的行数 */
  const parts = [];
  const tail = () => {
    if (n.thoughts) parts.push([count(n.thoughts, words.thoughts), false]);
    if (n.errors) parts.push([count(n.errors, words.errors), false]);
    if (took) parts.push([took, false]);
  };
  if (n.commands + n.agents + n.messages + n.tools + n.edits === 0) {
    // 只想过：本来就带时间；历史里不知道想了多久的，写几段思考
    if (known) return { spans: [{ text: words.thought_for.replace('{elapsed}', `${Math.max(1, Math.floor(thinking / 1000))}s`), tone: 'base' }], failed: false };
    tail();
    return { spans: join(parts, null), failed: false };
  }
  // 做事的只有一条命令（思考不算）、有短标题：短标题打头
  const only = n.commands === 1 && n.agents + n.messages + n.edits + n.tools === 0 ? steps.map((s) => arg(s, 'description')).find(Boolean) : null;
  if (only) {
    parts.push([only, false]);
    tail();
    return { spans: join(parts, null), failed: n.errors > 0 };
  }
  // 打头那一格：命令、子代理、留言、别的工具、编辑，先有哪样写哪样（派子代理的不写 Used 1 tool，蓝图「时间线」）；
  // 别的类跟在后面，打头那一格已经写过的类不再写一遍
  const lead = n.commands ? 'commands' : n.agents ? 'agents' : n.messages ? 'messages' : n.tools ? 'tools' : 'edits';
  const heads = { commands: words.ran, agents: words.spawned, messages: words.messaged, tools: words.used, edits: words.made };
  parts.push([count(n[lead], heads[lead]), lead === 'edits']);
  if (lead !== 'edits' && n.edits) parts.push([count(n.edits, words.edits), true]);
  if (lead !== 'agents' && n.agents) parts.push([count(n.agents, words.agents), false]);
  if (lead !== 'messages' && n.messages) parts.push([count(n.messages, words.messages), false]);
  if (lead !== 'tools' && n.tools) parts.push([count(n.tools, words.tools), false]);
  tail();
  return { spans: join(parts, changed(steps)), failed: false };
}

/**
 * 照说法换成一句话（照 `miyu-store` 的 `Human::say`）：没有这一句、少了字段的是 `null`；字段里的控制字符换成 `�`。
 * 模板里 `{名字}` 是字段，`{{`、`}}` 是括号本身（`miyu-kernel` 的 `template.rs`）。
 */
export function say(said) {
  const template = said ? res.human?.said?.[said.key] : null;
  if (!template) return null;
  const fields = said.fields ?? {};
  let missing = false;
  const text = template.replace(/\{\{|\}\}|\{([^{}]+)\}/g, (all, name) => {
    if (all === '{{') return '{';
    if (all === '}}') return '}';
    if (fields[name] == null) missing = true;
    return String(fields[name] ?? '').replace(/\p{Cc}/gu, '�');
  });
  return missing ? null : text;
}

/** 参数里的一个字符串，没有的是 `null`。 */
function arg(step, key) {
  const v = step.kind === 'tool' ? step.parsed?.[key] : null;
  return typeof v === 'string' ? v : null;
}

/** 参数写成一行一个 `键: 值`，嵌套的压成一行 JSON（照旧版 `app.js:8263-8294`）；`skip` 里的键不写；读不懂的照原文。 */
function pretty(step, skip = []) {
  const v = step.parsed;
  if (v == null) return step.args.trim();
  if (typeof v !== 'object' || Array.isArray(v)) return JSON.stringify(v, null, 2);
  return Object.entries(v).filter(([k]) => !skip.includes(k)).map(([k, raw]) => `${k}: ${raw == null ? '' : typeof raw === 'object' ? JSON.stringify(raw) : String(raw)}`).join('\n');
}

/** 这一段的编辑、写入一共加了几行、删了几行。 */
function changed(steps) {
  let added = 0;
  let removed = 0;
  for (const step of steps) {
    if (step.kind !== 'tool' || kindOf(step.name) !== 'edit' || unchanged(step)) continue;
    const d = fromArgs(step.parsed);
    added += d?.added ?? 0;
    removed += d?.removed ?? 0;
  }
  return { added, removed };
}

/** 一步用了多久（毫秒）：停了的照停表，还在进行的算到 `now`；不知道什么时候开始的是 `null`。 */
function elapsed(step, now) {
  if (step.start == null) return null;
  const end = step.end ?? (step.state === 'done' ? null : now);
  return end == null ? null : Math.max(0, end - step.start);
}

/** 这一段从第一步开始到最后一步结束的时长：只算知道时刻的那几步；一步都不知道的是 `null`。 */
function spanOf(steps, now) {
  const known = steps.filter((s) => elapsed(s, now) != null);
  if (!known.length) return null;
  const first = Math.min(...known.map((s) => /** @type {number} */ (s.start)));
  const last = Math.max(...known.map((s) => /** @type {number} */ (s.start) + /** @type {number} */ (elapsed(s, now))));
  return last - first;
}

/** 一格一格用 ` · ` 接起来；要接行数的那一格后面接 ` +3 -1`（都是 0 的不接）。 */
function join(parts, change) {
  /** @type {{text: string, tone: 'base'|'added'|'removed'}[]} */
  const out = [];
  parts.forEach(([text, withCounts], i) => {
    out.push({ text: i > 0 ? ` · ${text}` : text, tone: 'base' });
    if (withCounts && change && change.added + change.removed > 0) {
      out.push({ text: ' ', tone: 'base' }, { text: `+${change.added}`, tone: 'added' }, { text: ' ', tone: 'base' }, { text: `-${change.removed}`, tone: 'removed' });
    }
  });
  return out;
}
