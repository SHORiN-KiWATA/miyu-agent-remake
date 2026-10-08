// @ts-check
//! 后台任务的纯逻辑（`lib/`：纯的，谁都能用；蓝图 `web.md`「后台任务」「左栏」的「子代理的会话」）：从一个会话的事件里认出
//! 派出去的任务、在跑还是结束了、怎么排；挂在它下面的子代理。软件包 `jobs` 画后台任务的浮层，左栏画子代理的树，都照它。

/**
 * @typedef {{job: string, what: string, title: string, session: string|null, state: string, since: number|null, ended: number|null,
 *   duration: number|null, code: number|null, signal: number|null, command: string|null}} Task 一个任务（后台命令带着命令本身，照派它的
 *   那次调用的参数；子代理、读不懂的是 `null`）
 */

/** 一次调用的参数里的命令：读不懂的、没有的是 `null`。 */
function commandOf(raw) {
  try {
    const command = JSON.parse(raw ?? '{}').command;
    return typeof command === 'string' ? command : null;
  } catch {
    return null;
  }
}

/** 回报的原因 → 状态：命令照退出码分完成、失败；子代理 `done` 是完成。 */
function stateOf(e) {
  const b = e.body;
  if (e.kind === 'child.reported') return b.reason === 'done' ? 'done' : b.reason;
  if (b.reason !== 'exited') return b.reason;
  return b.signal == null && b.exit_code === 0 ? 'done' : 'failed';
}

/**
 * 子代理停在半路没有（蓝图 `web.md`「后台任务」第 5 条）：它的会话最后一轮是被打断的、之后没开新的一轮（核心的规矩：打断子会话的
 * 一轮，它闲着、不回报，等人说话或者被停，`agents.md` 第 212 行）。
 * @param {any[]} events 子代理的会话的事件
 */
export function paused(events) {
  const last = events.findLast((e) => e.kind === 'turn.started' || e.kind === 'turn.ended');
  return last?.kind === 'turn.ended' && last.body.reason === 'interrupted';
}

/**
 * 这个会话派出去的任务（`job.started`），照回报定状态；子代理报过以后又被留言叫醒的（`job.messaged`）算在跑，从留言那一刻算起。
 * 在跑的子代理，它的会话读进来了、停在半路的，算 `paused`。在跑的排前面，停在半路的接着，结束的排后面，各照编号排。
 * @param {any[]} events
 * @param {(session: string) => any[]|null} [child] 子代理的会话的事件（没读进来的是 `null`）
 * @returns {Task[]}
 */
export function tasksOf(events, child = () => null) {
  /** @type {Map<string, Task>} */
  const tasks = new Map();
  /** 调用编号 → 那次调用的参数（原样的 JSON） */
  const args = new Map();
  for (const e of events) {
    const at = Date.parse(e.at);
    // 还在跑的任务的种子（`core/store.js` 的 `SEED`，核心 9-6 上）：派它的那条在更早、还没读的页里时照它认；读到真的那条会把它盖掉
    if (e.kind === 'jobs.seed') {
      for (const x of e.body.jobs ?? []) {
        if (!tasks.has(x.job)) tasks.set(x.job, { job: x.job, what: x.what, title: x.title ?? '', session: x.session ?? null, state: 'running', since: null, ended: null, duration: null, code: null, signal: null, command: null });
      }
      continue;
    }
    if (e.kind === 'message.assistant') {
      for (const block of e.body.blocks ?? []) if (block.type === 'tool_call') args.set(block.call_id, block.args);
    } else if (e.kind === 'tool.result') {
      for (const fx of e.body.effects ?? []) {
        if (fx.kind === 'job.started') {
          const command = fx.what === 'command' ? commandOf(args.get(e.body.call_id)) : null;
          tasks.set(fx.job, { job: fx.job, what: fx.what, title: fx.title, session: fx.session ?? null, state: 'running', since: at, ended: null, duration: null, code: null, signal: null, command });
        } else if (fx.kind === 'job.messaged' && tasks.has(fx.job)) {
          const task = /** @type {Task} */ (tasks.get(fx.job));
          if (task.state !== 'running') Object.assign(task, { state: 'running', since: at, ended: null });
        }
      }
    } else if (e.kind === 'job.reported' || e.kind === 'child.reported') {
      const task = tasks.get(e.body.job);
      if (!task) continue;
      Object.assign(task, { state: stateOf(e), ended: at, duration: e.body.duration_ms ?? null, code: e.body.exit_code ?? null, signal: e.body.signal ?? null });
    }
  }
  const list = [...tasks.values()];
  for (const x of list) {
    const own = x.state === 'running' && x.what === 'agent' && x.session ? child(x.session) : null;
    if (own && paused(own)) x.state = 'paused';
  }
  const by = (state) => list.filter((x) => (state === 'done' ? x.state !== 'running' && x.state !== 'paused' : x.state === state)).sort((a, b) => compareJobs(a.job, b.job));
  return [...by('running'), ...by('paused'), ...by('done')];
}

/**
 * 挂在这个会话下面的子代理：派出去的 `agent`（带子会话编号），最新的在前；标题照派它时的 `title`，在跑的、停在半路的记着。
 * @param {any[]} events
 * @param {(session: string) => any[]|null} [child] 子代理的会话的事件（没读进来的是 `null`）
 */
export function childrenOf(events, child = () => null) {
  return tasksOf(events, child)
    .filter((x) => x.what === 'agent' && x.session)
    .sort((a, b) => compareJobs(b.job, a.job))
    .map((x) => ({ session: /** @type {string} */ (x.session), job: x.job, title: x.title, running: x.state === 'running', paused: x.state === 'paused' }));
}

/** 在跑的有几个。 */
export const running = (/** @type {Task[]} */ tasks) => tasks.filter((x) => x.state === 'running').length;

/**
 * 一个会话里在跑的后台任务一共几个（全部会话那一页、框下面那一行照它）：它派的，加上读进来了的子代理的会话再派的，一层层往下；
 * 同一个会话不数两遍（防着绕回来）。没读进来的会话是 0。
 * @param {string} id
 * @param {(session: string) => any[]|null} events 一个会话的事件（没读进来的是 `null`）
 * @param {Set<string>} [seen]
 */
export function runningDeep(id, events, seen = new Set()) {
  const own = events(id);
  if (!own || seen.has(id)) return 0;
  seen.add(id);
  const tasks = tasksOf(own, events);
  return running(tasks) + tasks.reduce((n, x) => n + (x.what === 'agent' && x.session ? runningDeep(x.session, events, seen) : 0), 0);
}

/** 任务编号照一段一段的数比（`j2` 在 `j2.1` 前面，`j2.9` 在 `j10` 前面；施工 7-1 补）。 */
export function compareJobs(a, b) {
  const pa = a.slice(1).split('.').map(Number);
  const pb = b.slice(1).split('.').map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    if (pa[i] == null) return -1;
    if (pb[i] == null) return 1;
    if (pa[i] !== pb[i]) return pa[i] - pb[i];
  }
  return 0;
}
