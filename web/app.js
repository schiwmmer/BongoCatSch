// The page shell: the rail, the focus page, the month grid and the clock loop.
// Everything the cultivation ladder draws lives in cultivate.js.

import { PomodoroWasm } from './wasm.js';
import {
  cache,
  emailOf,
  loadSave,
  saveSave,
  sendCode,
  session,
  signOut,
  verifyCode,
} from './cloud.js';
import {
  railCounts,
  renderCultivation,
  handleCultivationAction,
  CULTIVATION_TABS,
} from './cultivate.js';

const REALMS = {
  QiRefining: '炼气',
  Foundation: '筑基',
  GoldenCore: '金丹',
  NascentSoul: '元婴',
  ManaTransformation: '化神',
  VoidRefinement: '炼虚',
  Integration: '合体',
  Mahayana: '大乘',
  Tribulation: '渡劫',
  Immortal: '仙人',
};

const RAIL = [
  { group: '专注' },
  { id: 'focus', label: '番茄钟' },
  { group: '修为' },
  ...CULTIVATION_TABS.map((tab) => ({ id: tab.id, label: tab.label })),
];

const app = {
  wasm: null,
  snapshot: null,
  calendar: [],
  tab: 'focus',
  editingTask: null,
  calCursor: null,
  clockBase: 0,
  clockAt: 0,
  busy: false,
};

const stage = document.getElementById('stage');
const rail = document.getElementById('rail');
const topstats = document.getElementById('topstats');
const toasts = document.getElementById('toasts');

/* ---------- helpers ---------- */

const esc = (value) =>
  String(value ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);

const num = (value) => Number(value ?? 0).toLocaleString('zh-CN');

function clock(seconds) {
  const total = Math.max(0, Math.round(seconds));
  const mm = String(Math.floor(total / 60)).padStart(2, '0');
  const ss = String(total % 60).padStart(2, '0');
  return `${mm}:${ss}`;
}

export function describeError(error) {
  const raw = error?.message ?? error?.error_description ?? String(error ?? '未知错误');
  if (/origin/i.test(raw)) return '来源域名未授权，请从正式发布的地址打开';
  if (/network|fetch|Failed to fetch/i.test(raw)) return '网络不通，稍后自动重试';
  return raw;
}

function toast(text, kind = '') {
  const node = document.createElement('div');
  node.className = `toast ${kind}`.trim();
  node.textContent = text;
  toasts.appendChild(node);
  setTimeout(() => {
    node.style.transition = 'opacity .3s ease';
    node.style.opacity = '0';
    setTimeout(() => node.remove(), 320);
  }, 3200);
}

/* ---------- commands ---------- */

// Every write goes to the server. The local copy is refreshed at the same time
// so a cold start with no network can still open the timer, but it is never
// treated as the save: a failed cloud write is reported, not swallowed.
const sync = { pending: null, timer: null, retry: null, state: 'saved' };

function setSyncState(state) {
  sync.state = state;
  const node = document.getElementById('sync');
  if (!node) return;
  node.textContent = { saved: '已同步', saving: '同步中', offline: '未同步' }[state] ?? '';
  node.className = `sync ${state}`;
}

function persist() {
  if (!app.wasm) return;
  const text = app.wasm.export();
  cache.write(text);
  sync.pending = text;
  setSyncState('saving');
  clearTimeout(sync.timer);
  sync.timer = setTimeout(flushSave, 700);
}

async function flushSave() {
  const text = sync.pending;
  if (!text) return;
  sync.pending = null;
  try {
    await saveSave(text);
    setSyncState('saved');
    clearTimeout(sync.retry);
  } catch (error) {
    // Keep the text so the retry has something to send, and say so — the
    // record is not on the server yet.
    sync.pending = sync.pending ?? text;
    setSyncState('offline');
    clearTimeout(sync.retry);
    sync.retry = setTimeout(flushSave, 15000);
    toast(`云端保存失败：${describeError(error)}`, 'bad');
  }
}

// Apply a command, then redraw and save. Every button on the page goes through
// here, so "the screen changed" and "the save changed" cannot drift apart.
function send(payload, { quiet = false } = {}) {
  if (!app.wasm) return;
  const code = app.wasm.command(payload);
  if (code !== 0) {
    if (!quiet) toast('这个操作不被识别', 'bad');
    return false;
  }
  persist();
  refresh();
  return true;
}

/* ---------- data ---------- */

function refresh() {
  if (!app.wasm) return;
  app.snapshot = app.wasm.render();
  app.calendar = JSON.parse(readCalendar());
  const timer = app.snapshot?.timer;
  app.clockBase = timer ? timer.remaining_seconds : 0;
  app.clockAt = performance.now();
  render();
}

function readCalendar() {
  app.wasm.ex.web_calendar(PomodoroWasm.utcOffsetSeconds);
  return app.wasm.read();
}

/* ---------- rail + top bar ---------- */

function renderRail() {
  const counts = railCounts(app.snapshot);
  rail.innerHTML = RAIL.map((item) => {
    if (item.group) return `<div class="rail-group">${item.group}</div>`;
    const active = app.tab === item.id ? ' active' : '';
    const count = counts[item.id] ? `<span class="count">${counts[item.id]}</span>` : '';
    return `<button class="rail-item${active}" data-tab="${item.id}">${item.label}${count}</button>`;
  }).join('');
}

function renderTopStats() {
  const c = app.snapshot?.cultivation;
  if (!c) {
    topstats.innerHTML = '';
    return;
  }
  topstats.innerHTML = `
    <div class="stat"><span class="stat-label">境界</span><span class="stat-value">${REALMS[c.realm] ?? c.realm} · 第 ${c.layer + 1} 层第 ${c.rank} 阶</span></div>
    <div class="stat"><span class="stat-label">战力</span><span class="stat-value accent">${num(c.power)}</span></div>
    <div class="stat"><span class="stat-label">灵石</span><span class="stat-value gold">${num(c.stones)}</span></div>
    <div class="stat"><span class="stat-label">已专注</span><span class="stat-value jade">${app.snapshot.total_pomodoros ?? 0}</span></div>
  `;
}

/* ---------- focus page ---------- */

function timerView() {
  const timer = app.snapshot?.timer;
  const minutes = app.snapshot?.focus_minutes ?? 25;
  const total = timer ? timer.total_seconds : minutes * 60;
  const remaining = timer ? timer.remaining_seconds : minutes * 60;
  const ratio = total > 0 ? Math.max(0, Math.min(1, remaining / total)) : 0;
  const task = timer ? app.snapshot.tasks.find((t) => t.id === timer.task_id) : null;
  return { timer, total, remaining, ratio, task };
}

function dialMarkup() {
  const { timer, remaining, ratio, task } = timerView();
  const radius = 96;
  const circumference = 2 * Math.PI * radius;
  const offset = circumference * (1 - ratio);
  const stateClass = timer ? (timer.state === 'Running' ? '' : 'paused') : 'idle';
  const stateText =
    timer == null ? '未开始' : timer.state === 'Running' ? '专注中' : '已暂停';
  return `
    <div class="dial ${stateClass}" id="dial">
      <svg width="216" height="216" viewBox="0 0 216 216">
        <circle class="dial-track" cx="108" cy="108" r="${radius}" fill="none" stroke-width="10" />
        <circle class="dial-fill" id="dial-fill" cx="108" cy="108" r="${radius}" fill="none" stroke-width="10"
          stroke-dasharray="${circumference}" stroke-dashoffset="${offset}" />
      </svg>
      <div class="dial-center">
        <div class="clock" id="clock">${clock(remaining)}</div>
        <div class="dial-task">${esc(stateText)}${task ? ' · ' + esc(task.name) : ''}</div>
      </div>
    </div>
  `;
}

function timerActions() {
  const { timer } = timerView();
  if (!timer) {
    return `<span class="card-hint">从右侧任务里选一个开始</span>`;
  }
  if (timer.state === 'Running') {
    return `
      <button class="btn" data-act="pause">暂停</button>
      <button class="btn danger" data-act="abandon">放弃本次</button>
    `;
  }
  return `
    <button class="btn primary" data-act="resume">继续</button>
    <button class="btn danger" data-act="abandon">放弃本次</button>
  `;
}

function taskRow(task) {
  const editing = app.editingTask === task.id;
  const running = task.running ? ' running' : '';
  const done = task.status === 'done' ? ' done' : '';
  const pips = Array.from({ length: Math.min(task.estimated_pomodoros, 12) }, (_, index) =>
    `<i class="pip${index < task.completed_pomodoros ? ' on' : ''}"></i>`,
  ).join('');
  const overflow =
    task.estimated_pomodoros > 12
      ? `<span class="task-meta">+${task.estimated_pomodoros - 12}</span>`
      : '';
  if (editing) {
    return `
      <div class="task" data-id="${task.id}">
        <span></span>
        <div class="form-row" style="align-items:center">
          <input class="grow" type="text" id="edit-name" maxlength="80" value="${esc(task.name)}" />
          <input type="number" id="edit-est" min="1" max="99" value="${task.estimated_pomodoros}" style="width:74px" />
        </div>
        <div class="task-actions">
          <button class="btn small jade" data-act="save-task" data-id="${task.id}">保存</button>
          <button class="btn small ghost" data-act="cancel-edit">取消</button>
        </div>
      </div>
    `;
  }
  return `
    <div class="task${running}${done}" data-id="${task.id}">
      <input class="check" type="checkbox" data-act="toggle-task" data-id="${task.id}" ${task.status === 'done' ? 'checked' : ''} />
      <div>
        <div class="task-name">${esc(task.name)}</div>
        <div class="task-line">
          <span class="task-meta">${task.completed_pomodoros}/${task.estimated_pomodoros} 番茄</span>
          <span class="pips">${pips}${overflow}</span>
          ${task.running ? '<span class="chip accent">进行中</span>' : ''}
        </div>
      </div>
      <div class="task-actions">
        <button class="btn small${task.running ? '' : ' primary'}" data-act="start-task" data-id="${task.id}" ${task.running ? 'disabled' : ''}>开始</button>
        <button class="btn small ghost" data-act="edit-task" data-id="${task.id}">编辑</button>
        <button class="btn small ghost danger" data-act="delete-task" data-id="${task.id}">删除</button>
      </div>
    </div>
  `;
}

function calendarMarkup() {
  const now = new Date();
  const cursor = app.calCursor ?? { year: now.getFullYear(), month: now.getMonth() };
  const first = new Date(cursor.year, cursor.month, 1);
  const daysInMonth = new Date(cursor.year, cursor.month + 1, 0).getDate();
  const leading = first.getDay();
  const byDay = new Map();
  for (const entry of app.calendar) {
    // Day numbers count from 1970-01-01 UTC, so a UTC date is the local date
    // this page is asking about.
    const date = new Date(entry.day * 86400000);
    if (date.getUTCFullYear() === cursor.year && date.getUTCMonth() === cursor.month) {
      byDay.set(date.getUTCDate(), entry);
    }
  }
  const todayKey =
    now.getFullYear() === cursor.year && now.getMonth() === cursor.month ? now.getDate() : -1;
  const cells = [];
  for (let i = 0; i < leading; i += 1) cells.push('<div class="cal-cell blank"></div>');
  let total = 0;
  for (let day = 1; day <= daysInMonth; day += 1) {
    const entry = byDay.get(day);
    const count = entry?.pomodoros ?? 0;
    total += count;
    const level = count === 0 ? 0 : count < 2 ? 1 : count < 4 ? 2 : count < 6 ? 3 : 4;
    const cls = ['cal-cell', level ? `l${level}` : '', day === todayKey ? 'today' : ''].filter(Boolean).join(' ');
    cells.push(
      `<div class="${cls}" title="${count} 个番茄"><span class="n">${day}</span>${count ? `<span class="c">${count}</span>` : ''}</div>`,
    );
  }
  return `
    <div class="card">
      <div class="card-head">
        <div class="card-title">专注日历</div>
        <div class="wrap" style="gap:6px">
          <button class="btn small ghost" data-act="cal-prev">‹</button>
          <span class="card-hint" style="min-width:96px;text-align:center">${cursor.year} 年 ${cursor.month + 1} 月 · ${total} 个</span>
          <button class="btn small ghost" data-act="cal-next">›</button>
        </div>
      </div>
      <div class="cal">
        ${['日', '一', '二', '三', '四', '五', '六'].map((d) => `<div class="cal-head">${d}</div>`).join('')}
        ${cells.join('')}
      </div>
    </div>
  `;
}

function focusPage() {
  const snapshot = app.snapshot;
  const { timer } = timerView();
  const tasks = snapshot.tasks;
  const open = tasks.filter((t) => t.status !== 'done');
  const focusMinutes = snapshot.focus_minutes;
  return `
    <div class="grid-2">
      <div style="display:flex;flex-direction:column;gap:16px">
      <div class="card timer-card">
        ${dialMarkup()}
        <div class="timer-actions">${timerActions()}</div>
        <div class="timer-meta">
          <div class="stat"><span class="stat-label">每次时长</span><span class="stat-value">${focusMinutes} 分</span></div>
          <div class="stat"><span class="stat-label">今日</span><span class="stat-value jade">${todayCount()} 个</span></div>
          <div class="stat"><span class="stat-label">待完成</span><span class="stat-value">${snapshot.remaining_pomodoros} 个</span></div>
        </div>
      </div>
      ${calendarMarkup()}
      </div>

      <div style="display:flex;flex-direction:column;gap:16px">
        <div class="card">
          <div class="card-head">
            <div class="card-title">新建任务</div>
            <div class="card-hint">每完成一次专注，修为增长一次</div>
          </div>
          <div class="form-row">
            <div class="field grow">
              <span class="field-label">任务名</span>
              <input type="text" id="new-name" maxlength="80" placeholder="要专注的事，例如：写番茄钟网站" />
            </div>
            <div class="field" style="width:96px">
              <span class="field-label">预计番茄</span>
              <input type="number" id="new-est" min="1" max="99" value="4" />
            </div>
            <div class="field" style="width:96px">
              <span class="field-label">每次分钟</span>
              <input type="number" id="new-minutes" min="1" max="120" value="${focusMinutes}" />
            </div>
            <button class="btn primary" data-act="create-task">添加</button>
          </div>
        </div>

        <div class="card">
          <div class="card-head">
            <div class="card-title">任务</div>
            <div class="card-hint">${open.length} 个进行中 · ${tasks.length - open.length} 个已完成</div>
          </div>
          <div class="task-list">
            ${tasks.length ? tasks.map(taskRow).join('') : '<div class="empty">还没有任务，先添加一个。</div>'}
          </div>
        </div>
      </div>
    </div>
  `;
}

function todayCount() {
  const now = new Date();
  const todayNumber = Math.floor(
    Date.UTC(now.getFullYear(), now.getMonth(), now.getDate()) / 86400000,
  );
  return app.calendar.find((entry) => entry.day === todayNumber)?.pomodoros ?? 0;
}

/* ---------- clock ---------- */

// The countdown is repainted from the state the snapshot carried, minus the
// time since that snapshot: the wasm side owns the truth, this only smooths it
// between the once-a-second reads.
function paintClock() {
  const clockNode = document.getElementById('clock');
  const fill = document.getElementById('dial-fill');
  const timer = app.snapshot?.timer;
  if (!clockNode || !fill || !timer) return;
  if (timer.state !== 'Running') return;
  const elapsed = (performance.now() - app.clockAt) / 1000;
  const remaining = Math.max(0, app.clockBase - elapsed);
  clockNode.textContent = clock(remaining);
  const total = timer.total_seconds || 1;
  const ratio = Math.max(0, Math.min(1, remaining / total));
  const radius = 96;
  const circumference = 2 * Math.PI * radius;
  fill.setAttribute('stroke-dashoffset', String(circumference * (1 - ratio)));
}

/* ---------- render ---------- */

function render() {
  if (!app.snapshot) return;
  renderRail();
  renderTopStats();
  if (app.tab === 'focus') {
    stage.innerHTML = focusPage();
    return;
  }
  stage.innerHTML = renderCultivation(app.tab, app.snapshot, { send, toast });
}

/* ---------- events ---------- */

stage.addEventListener('click', (event) => {
  const tabButton = event.target.closest('[data-tab]');
  if (tabButton) {
    app.tab = tabButton.dataset.tab;
    render();
    return;
  }
  const button = event.target.closest('[data-act]');
  if (!button) return;
  // A checkbox fires on click too; its own handler reads the new state.
  if (button.dataset.act === 'toggle-task') return;
  handleAction(button.dataset, button);
});

stage.addEventListener('change', (event) => {
  const target = event.target;
  if (target.dataset?.act === 'toggle-task') {
    send({ op: 'set_task_done', task_id: Number(target.dataset.id), done: target.checked });
  }
});

rail.addEventListener('click', (event) => {
  const button = event.target.closest('[data-tab]');
  if (!button) return;
  app.tab = button.dataset.tab;
  render();
});

function handleAction(data, button) {
  const id = Number(data.id ?? 0);
  switch (data.act) {
    case 'create-task': {
      const name = document.getElementById('new-name').value.trim();
      const est = Number(document.getElementById('new-est').value) || 1;
      const minutes = Number(document.getElementById('new-minutes').value) || 25;
      if (!name) {
        toast('给任务起个名字', 'bad');
        return;
      }
      if (app.snapshot.focus_minutes !== minutes) {
        send({ op: 'set_focus_minutes', minutes });
      }
      if (send({ op: 'create_task', name, estimated: Math.min(99, Math.max(1, est)) })) {
        toast('任务已添加', 'good');
      }
      return;
    }
    case 'start-task':
      send({ op: 'start_task', task_id: id });
      return;
    case 'pause':
      send({ op: 'pause' });
      return;
    case 'resume':
      send({ op: 'resume' });
      return;
    case 'abandon':
      send({ op: 'abandon' });
      toast('本次专注已放弃');
      return;
    case 'edit-task':
      app.editingTask = id;
      render();
      document.getElementById('edit-name')?.focus();
      return;
    case 'cancel-edit':
      app.editingTask = null;
      render();
      return;
    case 'save-task': {
      const name = document.getElementById('edit-name').value.trim();
      const est = Number(document.getElementById('edit-est').value) || 1;
      if (!name) {
        toast('名字不能为空', 'bad');
        return;
      }
      app.editingTask = null;
      send({ op: 'update_task', task_id: id, name, estimated: Math.min(99, Math.max(1, est)) });
      return;
    }
    case 'delete-task':
      app.editingTask = null;
      send({ op: 'delete_task', task_id: id });
      toast('任务已删除');
      return;
    case 'cal-prev':
    case 'cal-next': {
      const now = new Date();
      const cursor = app.calCursor ?? { year: now.getFullYear(), month: now.getMonth() };
      const delta = data.act === 'cal-prev' ? -1 : 1;
      const month = cursor.month + delta;
      app.calCursor = { year: cursor.year + Math.floor(month / 12), month: ((month % 12) + 12) % 12 };
      render();
      return;
    }
    default:
      // The cultivation page owns its own verbs.
      handleCultivationAction(data, send, toast);
  }
}

/* ---------- boot ---------- */

/* ---------- gate, account, boot ---------- */

const gate = document.getElementById('gate');
const gateForm = document.getElementById('gate-form');
const gateEmail = document.getElementById('gate-email');
const gateCode = document.getElementById('gate-code');
const gateSend = document.getElementById('gate-send');
const gateSubmit = document.getElementById('gate-submit');
const gateError = document.getElementById('gate-error');
const account = document.getElementById('account');
const accountEmail = document.getElementById('account-email');

// The code is issued once and reused for the submit: sending again would spend
// another mail and invalidate the one the user is reading.
let pendingOtp = null;
let sendCooldown = 0;

function setGateError(text) {
  gateError.textContent = text;
}

function showGate() {
  gate.hidden = false;
  account.hidden = true;
  stage.innerHTML = '<div class="boot">登录后开始专注</div>';
  rail.innerHTML = '';
  topstats.innerHTML = '';
  window.setTimeout(() => gateEmail.focus(), 60);
}

function hideGate() {
  gate.hidden = true;
}

function renderAccount(current) {
  account.hidden = false;
  accountEmail.textContent = emailOf(current);
  setSyncState(sync.state);
}

gateSend.addEventListener('click', async () => {
  const email = gateEmail.value.trim();
  if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email)) {
    setGateError('请填写正确的邮箱');
    return;
  }
  if (Date.now() < sendCooldown) {
    setGateError(`请 ${Math.ceil((sendCooldown - Date.now()) / 1000)} 秒后再获取`);
    return;
  }
  gateSend.disabled = true;
  setGateError('');
  try {
    const data = await sendCode(email);
    pendingOtp = {
      email,
      verificationId: data.verificationId,
      isExistingUser: data.isExistingUser,
    };
    gateCode.focus();
    toast('验证码已发送，请查收邮件', 'good');
    sendCooldown = Date.now() + 60_000;
    const tick = window.setInterval(() => {
      const left = Math.ceil((sendCooldown - Date.now()) / 1000);
      gateSend.textContent = left > 0 ? `${left}s` : '重新获取';
      if (left <= 0) {
        window.clearInterval(tick);
        gateSend.disabled = false;
      }
    }, 1000);
  } catch (error) {
    setGateError(describeError(error));
    gateSend.disabled = false;
  }
});

gateForm.addEventListener('submit', async (event) => {
  event.preventDefault();
  const email = gateEmail.value.trim();
  const token = gateCode.value.trim();
  if (!pendingOtp || pendingOtp.email !== email) {
    setGateError('请先获取这个邮箱的验证码');
    return;
  }
  if (!token) {
    setGateError('请输入验证码');
    return;
  }
  gateSubmit.disabled = true;
  setGateError('');
  try {
    await verifyCode({ ...pendingOtp, token });
    pendingOtp = null;
    const current = await session();
    if (!current) throw new Error('登录状态未建立，请重试');
    await startSignedIn(current);
  } catch (error) {
    setGateError(describeError(error));
  } finally {
    gateSubmit.disabled = false;
  }
});

document.getElementById('sign-out').addEventListener('click', async () => {
  await flushSave();
  await signOut();
  // Drop the cache too: leaving another account's record on this machine would
  // let the next sign-in open someone else's save.
  cache.clear();
  window.location.reload();
});

async function startSignedIn(current) {
  hideGate();
  renderAccount(current);
  let documentText = '';
  let seeded = false;
  try {
    const remote = await loadSave();
    if (remote) {
      documentText = remote.document;
    } else {
      // First sign-in for this account: carry over whatever this browser had
      // before the save moved to the server, so nothing is lost in the move.
      documentText = cache.read();
      seeded = Boolean(documentText);
    }
  } catch (error) {
    documentText = cache.read();
    setSyncState('offline');
    toast(`读取云端存档失败，先打开本地副本：${describeError(error)}`, 'bad');
  }

  app.wasm.boot(documentText);
  refresh();
  if (seeded) {
    persist();
    toast('已把本机的记录上传到云端', 'good');
  }

  let last = performance.now();
  window.setInterval(() => {
    if (!app.wasm) return;
    const now = performance.now();
    const elapsed = Math.min(5000, now - last);
    last = now;
    const result = app.wasm.tick(elapsed);
    if (result & 2) {
      persist();
      refresh();
      return;
    }
    if (result & 1) {
      persist();
      refresh();
      toast('一次专注完成，修为 +1', 'good');
      return;
    }
    paintClock();
  }, 250);

  window.setInterval(() => {
    if (app.tab === 'focus') {
      const node = document.querySelector('.timer-meta .stat-value.jade');
      if (node) node.textContent = `${todayCount()} 个`;
    }
  }, 5000);
}

async function main() {
  try {
    app.wasm = await PomodoroWasm.load('./bongocat_pomodoro_web.wasm');
  } catch (error) {
    stage.innerHTML = `<div class="card"><div class="card-title">wasm 载入失败</div><p class="card-hint">${esc(error.message)}<br />请通过 http 服务打开这个页面，而不是 file:// 直接双击。</p></div>`;
    return;
  }

  let current = null;
  try {
    current = await session();
  } catch (error) {
    stage.innerHTML = `<div class="card"><div class="card-title">无法连接云端</div><p class="card-hint">${esc(describeError(error))}</p></div>`;
    return;
  }
  if (!current) {
    showGate();
    return;
  }
  await startSignedIn(current);
}

main();
