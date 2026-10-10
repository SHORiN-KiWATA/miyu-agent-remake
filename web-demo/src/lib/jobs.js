// @ts-check
//! 后台任务的纯逻辑（`lib/`：纯的，谁都能用；蓝图 `web.md`「后台任务」「左栏」的「子代理的会话」）：照会话状态的任务表（核心 9-8）
//! 认出派出去的任务、在跑还是结束了、怎么排；挂在它下面的子代理。软件包 `jobs` 画后台任务的浮层，左栏画子代理的树，都照它。

/**
 * @typedef {{job: string, what: string, title: string, session: string|null, state: string, since: number|null, ended: number|null,
 *   duration: number|null, code: number|null, signal: number|null, command: string|null}} Task 一个任务（后台命令带着命令本身；
 *   子代理的是 `null`）
 */

/** 在跑的有几个。 */
export const running = (/** @type {Task[]} */ tasks) => tasks.filter((x) => x.state === 'running').length;

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

/**
 * 会话状态的任务表（核心 9-8 补上的 `view.status` 的 `jobs`，蓝图 `web.md`「照条目画」第 3 条）换成上面那一种：`stopped` 的照 `why`
 * （人停、随撤销、因重启）写，别的照 `state`。子代理的 `done` 是它那一轮结束了（还能留言叫醒）。在跑的排前面，结束的排后面，各照编号
 * 从新到旧。
 * @param {any[]|null|undefined} jobs
 * @returns {Task[]}
 */
export function tasksFromStatus(jobs) {
  const at = (/** @type {string|undefined} */ t) => (t ? Date.parse(t) : null);
  const list = (jobs ?? []).map((/** @type {any} */ j) => {
    const since = at(j.started);
    const ended = at(j.ended);
    return {
      job: j.job, what: j.what, title: j.title ?? '', session: j.session ?? null,
      state: j.state === 'stopped' ? j.why ?? 'stopped' : j.state,
      since, ended, duration: since != null && ended != null ? ended - since : null,
      code: j.exit_code ?? null, signal: j.signal ?? null, command: j.command ?? null,
    };
  });
  const by = (/** @type {string} */ state) => list.filter((x) => (state === 'running' ? x.state === 'running' : x.state !== 'running')).sort((a, b) => compareJobs(b.job, a.job));
  return [...by('running'), ...by('done')];
}

/** 照会话状态挂在这个会话下面的子代理（左栏那棵树）。 @param {any[]|null|undefined} jobs */
export function childrenFromStatus(jobs) {
  return tasksFromStatus(jobs)
    .filter((x) => x.what === 'agent' && x.session)
    .sort((a, b) => compareJobs(b.job, a.job))
    .map((x) => ({ session: /** @type {string} */ (x.session), job: x.job, title: x.title, running: x.state === 'running', paused: false }));
}

/**
 * 照会话状态数一个会话里在跑的后台任务：核心给了整棵树的（`running_deep`，9-8 补下）照它，没有的连读进来了的子代理再派的一层层往下数。
 * @param {string} id @param {(session: string) => any|null} statusOf 一个会话的会话状态（没读进来的是 `null`） @param {Set<string>} [seen]
 */
export function runningDeepStatus(id, statusOf, seen = new Set()) {
  const status = statusOf(id);
  if (!status || seen.has(id)) return 0;
  // 核心算好了整棵树（9-8 补下）：照它
  if (typeof status.running_deep === 'number') return status.running_deep;
  seen.add(id);
  const tasks = tasksFromStatus(status.jobs);
  return running(tasks) + tasks.reduce((n, x) => n + (x.what === 'agent' && x.session ? runningDeepStatus(x.session, statusOf, seen) : 0), 0);
}
