// 快照 → 页面数据。所有字符串都在这里拼好，WXML 只负责摆放。
//
// 为什么要多这一层：WXML 里不能调用 JS 函数，模板里做格式化会把判断逻辑散进
// 十几个 `wx:if`。这里一次性把「该显示什么话」算完，页面只读字段。

const L = require('./labels');

const SET_EFFECTS = {
  frost: '寒霜',
  thunder: '雷亟',
  sunder: '碎甲',
  weaken: '蛊蚀',
};

// 一次闭关的回合数，内核常量 `SECLUSION_TICKS`。快照没带这个总数，先按它写死。
const SECLUSION_TICKS = 250;
const JOB_GRADE_NAMES = ['黄阶', '玄阶', '地阶', '天阶', '星阶'];
const BATTLE_KINDS = { Trial: '锁妖塔之战', Proving: '试炼之战', Breakthrough: '破境之战' };
// 采集点的叫法，与 App 的 `cultivation.site.*` 逐字一致。`L.SITES` 是任务行用的
// 简称（森林/山麓/矿洞），通知行沿用 App 的「森林·伐木」口径。
const COLLECT_SITES = { Forest: '森林·伐木', Mountain: '山·采石', Mine: '矿洞·双技能' };

/// 当前任务那一行：任务 > 闭关 > 战斗 > 空闲。全部读快照，不在这里算数。
function taskText(c) {
  const task = c.task;
  if (task) {
    const name = L.TASK_KINDS[task.kind] || task.kind || '任务';
    const site = task.site ? L.SITES[task.site] || task.site : '';
    const tier = task.tier && task.tier !== 'Common' ? L.GATHER_TIERS[task.tier] || '' : '';
    const place = [site, tier].filter(Boolean).join(' · ');
    const grade = task.grade != null ? JOB_GRADE_NAMES[task.grade] || '' : '';
    const detail = place || grade;
    const ticks = `${L.num(task.ticks_done)} 回合 · 每回合 ${task.tick_seconds} 秒`;
    return detail ? `${name} · ${detail} · 已进行 ${ticks}` : `${name} · 已进行 ${ticks}`;
  }
  if (c.seclusion) {
    return `闭关 · 回合 ${L.num(c.seclusion.ticks_done)}/${SECLUSION_TICKS}`;
  }
  const battle = c.battle;
  if (battle) {
    const name = BATTLE_KINDS[battle.kind] || battle.kind || '战斗';
    const floor = battle.kind === 'Trial' && battle.floor ? ` · 第 ${battle.floor} 层` : '';
    return `${name}${floor} · 回合 ${battle.rounds_done}/${battle.rounds_total}`;
  }
  return '空闲';
}

/// 最近一次采集那一行：满仓时说出被仓库吞掉的那一份，而不是一行「获得 」让人
/// 以为渲染坏了。口径与 App 的 `cultivation.notice.collect*` 一致。
function collectText(collect) {
  const site = COLLECT_SITES[collect.site] || collect.site;
  const bagText = (bag) =>
    L.RESOURCES.filter(([key]) => ((bag && bag[key]) || 0) > 0)
      .map(([key, label]) => `${label}×${L.num(bag[key])}`)
      .join(' · ');
  const lost = collect.lost && Object.values(collect.lost).some((n) => n > 0);
  const gained = bagText(lost ? collect.lost : collect.gained) || '空手而归';
  const xp = (collect.woodcut_xp || 0) + (collect.stonecut_xp || 0);
  return lost
    ? `${site}：仓库已满，${gained} 未能入袋，采集经验 +${xp}`
    : `${site}：获得 ${gained}，采集经验 +${xp}`;
}

/* ---------- 通用 ---------- */

function header(snapshot) {
  const c = (snapshot && snapshot.cultivation) || {};
  return {
    realm: `${L.realmText(c.realm)} · ${(c.layer || 0) + 1} 层 ${c.rank || 1} 阶`,
    power: L.num(c.power),
    stones: L.num(c.stones),
    total: String(snapshot ? snapshot.total_pomodoros || 0 : 0),
    atHome: !!c.at_home,
  };
}

function bar(ratio) {
  const value = Math.max(0, Math.min(1, Number(ratio) || 0));
  return Math.round(value * 100);
}

// 一个回合的填充进度，0..1000。内核只在整回合结算时发布，两次发布之间快照不动，
// 所以「这一回合走了多远」按快照的 `start_unix` 和当前时钟自己读 —— 与 App 端
// `ActiveTask::progress_permille` / `BattleRun::total_progress_permille` 同口径。
// `now` 是毫秒：结算边是整秒，按秒读一条条只能一秒一跳。`start_unix` 缺失时读
// 0：宁可空着，也不假装快到。
function tickPermille(startUnix, tickSeconds, now) {
  const start = Number(startUnix);
  if (!start) return 0;
  const tick = Math.max(1, tickSeconds || 1) * 1000;
  const elapsed = Math.max(0, now - start * 1000);
  return Math.min(1000, Math.floor((elapsed * 1000) / tick));
}

// 当前任务的实时进度：任务读自己那一回合；战斗读整场，不读当前这一回合——一
// 回合一秒，读单回合会画成每秒归零的秒表。没有进行中的一项是 null。
function liveProgress(c, now) {
  const task = c.task ? tickPermille(c.task.start_unix, c.task.tick_seconds, now) : null;
  const battle = c.battle;
  const fight = battle
    ? Math.min(
        1000,
        Math.floor(
          ((battle.rounds_done || 0) * 1000 +
            tickPermille(battle.start_unix, battle.tick_seconds, now)) /
            Math.max(1, battle.rounds_total || 1),
        ),
      )
    : null;
  return { task, battle: fight };
}

/* ---------- 番茄钟 ---------- */

function calendarView(entries, cursor) {
  const now = new Date();
  const year = cursor && cursor.year != null ? cursor.year : now.getFullYear();
  const month = cursor && cursor.month != null ? cursor.month : now.getMonth();
  const daysInMonth = new Date(year, month + 1, 0).getDate();
  const leading = new Date(year, month, 1).getDay();
  const isCurrentMonth = now.getFullYear() === year && now.getMonth() === month;

  const byDay = {};
  (entries || []).forEach((entry) => {
    const date = new Date(entry.day * 86400000);
    if (date.getUTCFullYear() === year && date.getUTCMonth() === month) {
      byDay[date.getUTCDate()] = entry.pomodoros || 0;
    }
  });

  const cells = [];
  for (let index = 0; index < leading; index += 1) {
    cells.push({ key: `blank-${index}`, blank: true });
  }
  let total = 0;
  for (let day = 1; day <= daysInMonth; day += 1) {
    const count = byDay[day] || 0;
    total += count;
    const level = count === 0 ? 0 : count < 2 ? 1 : count < 4 ? 2 : count < 6 ? 3 : 4;
    cells.push({
      key: `day-${day}`,
      day,
      count: count || '',
      level,
      today: isCurrentMonth && day === now.getDate(),
    });
  }
  return {
    title: `${year} 年 ${month + 1} 月`,
    total,
    year,
    month,
    week: ['日', '一', '二', '三', '四', '五', '六'],
    cells,
  };
}

function todayCount(entries) {
  const now = new Date();
  const todayNumber = Math.floor(Date.UTC(now.getFullYear(), now.getMonth(), now.getDate()) / 86400000);
  const found = (entries || []).find((entry) => entry.day === todayNumber);
  return found ? found.pomodoros || 0 : 0;
}

function pomodoro(snapshot, calendar, cursor, remaining) {
  const timer = snapshot.timer;
  const minutes = snapshot.focus_minutes || 25;
  const total = timer ? timer.total_seconds : minutes * 60;
  const left = typeof remaining === 'number' ? remaining : timer ? timer.remaining_seconds : total;
  const task = timer ? (snapshot.tasks || []).find((item) => item.id === timer.task_id) : null;
  const running = !!timer && timer.state === 'Running';

  return {
    header: header(snapshot),
    hasTimer: !!timer,
    running,
    totalSeconds: total,
    stateText: !timer ? '未开始' : running ? '专注中' : '已暂停',
    taskName: task ? task.name : '',
    clock: L.clock(left),
    ratio: bar(total > 0 ? left / total : 0),
    actions: !timer
      ? []
      : running
        ? [
            { act: 'pause', label: '暂停', kind: 'primary' },
            { act: 'abandon', label: '放弃', kind: 'danger' },
          ]
        : [
            { act: 'resume', label: '继续', kind: 'primary' },
            { act: 'abandon', label: '放弃', kind: 'danger' },
          ],
    minutes,
    today: todayCount(calendar),
    pending: snapshot.remaining_pomodoros || 0,
    tasks: (snapshot.tasks || []).map((item) => {
      const pips = [];
      const shown = Math.min(item.estimated_pomodoros, 12);
      for (let index = 0; index < shown; index += 1) {
        pips.push({ key: index, on: index < item.completed_pomodoros });
      }
      return {
        id: item.id,
        name: item.name,
        done: item.status === 'done',
        running: !!item.running,
        countText: `${item.completed_pomodoros}/${item.estimated_pomodoros}`,
        pips,
        overflow: item.estimated_pomodoros > 12 ? item.estimated_pomodoros - 12 : 0,
      };
    }),
    cal: calendarView(calendar, cursor),
    minuteOptions: [25, 30, 45, 60].map((value) => ({ value, active: value === minutes })),
    // What the next finished pomodoro is for, chosen where the session starts.
    // The same three modes and the same command as the other surfaces, so a
    // choice made here shows up everywhere else.
    modes: L.MODES.map(([key, label]) => ({
      key: key.toLowerCase(),
      label,
      active: snapshot.cultivation?.session_mode === key,
    })),
    // 采集按钮下面的存货：伐木/采石能把哪个袋子填多少，看的就是这一行。
    purse: L.RESOURCES.map(
      ([key, label]) => `${label} ${L.num((snapshot.cultivation?.resources?.bag || {})[key] || 0)}`,
    ).join(' · '),
  };
}

/* ---------- 修仙 · 外出 ---------- */

function outing(c) {
  const regionIndex = c.explore_region_idx || 0;
  const region = L.REGIONS[regionIndex] || ['', `区域 ${regionIndex + 1}`];
  const regionProgress = (c.region_progress && c.region_progress[regionIndex]) || 0;

  const regions = (c.explored_levels || []).map((cleared, index) => {
    const def = L.REGIONS[index] || ['', `区域 ${index + 1}`];
    return {
      key: def[0] || `region-${index}`,
      name: def[1],
      cleared,
      percent: bar(((c.region_progress && c.region_progress[index]) || 0) / 1000),
      current: index === regionIndex,
      level: index === regionIndex ? c.explore_level || 1 : null,
    };
  });

  const jobs = (c.jobs || []).map((job) => {
    const first = L.DERIVED_NAMES[job.kind.first] || job.kind.first;
    const second = job.kind.second ? L.DERIVED_NAMES[job.kind.second] || job.kind.second : '';
    return {
      slot: job.slot,
      grade: L.gradeText(job.grade),
      kind: second && second !== first ? `${first} / ${second}` : first,
      requirement: `${L.num(job.requirement)}${job.requirement_second ? ` / ${L.num(job.requirement_second)}` : ''}`,
      current: `${L.num(job.current_first)}${job.current_second ? ` / ${L.num(job.current_second)}` : ''}`,
      rate: Math.round(job.success_rate / 10),
      wage: L.num(job.hourly_stones),
      working: !!job.working,
      disabled: !!job.working || !c.at_home,
    };
  });

  // 正在进行的打工任务：S4 之后 `accept_job` 启动的是 Work 任务，旧档的
  // `work` 字段为空。页面上「提前收工」要能停它，所以这里认任务形态。
  const workTask = c.task && c.task.kind === 'Work' ? c.task : null;

  const found = c.last_exploration_found;
  const chips = [];
  if (found) {
    if (found.seed) chips.push({ key: 'seed', text: `种子${found.seed.name ? ` ${found.seed.name}` : ''}` });
    if (found.gear) chips.push({ key: 'gear', text: found.gear.name || '装备' });
    if (found.art) chips.push({ key: 'art', text: `功法 ${found.art}` });
    if (found.stones) chips.push({ key: 'stones', text: `${L.num(found.stones)} 灵石` });
  }

  return {
    statusText: c.at_home
      ? '在家，可以出门'
      : `人在外面，${c.outing_return_seconds ? `${L.duration(c.outing_return_seconds)}后回来` : '归期未定'}`,
    regionName: region[1],
    regionLevel: c.explore_level || 1,
    regionPercent: (regionProgress / 10).toFixed(1),
    explorationReady: !!c.exploration_ready,
    cooldownText: c.exploration_ready_at ? `探索冷却，还需 ${L.until(c.exploration_ready_at)}` : '',
    hours: [4, 8, 12].map((value) => ({ value, primary: value === 8 })),
    earlyReturn: !!(c.walk_early || c.work_early),
    found: chips,
    regions,
    jobs,
    // S4 之后打工是实时任务（`accept_job` 启动 Work 任务），旧档的 hour-job
    // 字段不再有值；「提前收工」两种都要能停，所以这里把 Work 任务也算上。
    workText: c.work
      ? `正在干活，${L.until(c.work.ends_unix)}后收工`
      : workTask
        ? `正在干活，已进行 ${L.num(workTask.ticks_done)} 回合`
        : `刷新一次 ${L.num(c.job_refresh_cost)} 灵石`,
    work: !!c.work || !!workTask,
    refreshCost: L.num(c.job_refresh_cost),
    lastWork: c.last_work ? `上次工钱 ${L.num(c.last_work.stones || 0)} 灵石` : '',
  };
}

/* ---------- 修仙 · 塔与试炼 ---------- */

function tower(c) {
  const attempts = c.trial_attempts_left || 0;
  // The last 试炼's find, as the same chips a walk's find wears: a loss finds
  // nothing and shows none, and a win that moved the region names the percent
  // it moved beside what it turned up.
  const proving = c.last_proving;
  const provingChips = [];
  if (proving && proving.found) {
    const found = proving.found;
    if (found.seed) {
      provingChips.push({ key: 'seed', text: `种子${found.seed.name ? ` ${found.seed.name}` : ''}` });
    }
    if (found.gear) provingChips.push({ key: 'gear', text: found.gear.name || '装备' });
    if (found.art) provingChips.push({ key: 'art', text: `功法 ${found.art}` });
    if (found.stones) {
      provingChips.push({ key: 'stones', text: `${L.num(found.stones)} 灵石` });
    }
  }
  if (proving && proving.progress != null) {
    provingChips.push({
      key: 'progress',
      text: `试炼 +${(proving.progress / 10).toFixed(1)}%`,
    });
  }
  return {
    floor: c.trial_floor || 0,
    attempts,
    nextFloor: c.trial_next_floor || 1,
    wardenPower: L.num(c.trial_warden_power),
    power: L.num(c.power),
    canChallenge: attempts > 0 && !!c.at_home,
    canSweep: !!c.trial_sweepable && attempts > 0 && !!c.at_home,
    sweepable: !!c.trial_sweepable,
    blockedText: c.trial_blocked ? L.refusalText(c.trial_blocked) : '',
    tokens: c.proving_tokens || 0,
    tokenCap: c.proving_token_cap || 10,
    canProve: (c.proving_tokens || 0) > 0 && !!c.at_home,
    provingBlocked: c.proving_blocked ? L.refusalText(c.proving_blocked) : '',
    provingChips,
    record: (c.duel_record || [])
      .slice(-8)
      .reverse()
      .map((duel, index) => ({
        key: `${duel.at_unix}-${index}`,
        won: !!duel.won,
        kind: L.FIGHTS[duel.kind] || duel.kind,
        realm: L.realmText(duel.realm),
        at: L.when(duel.at_unix),
      })),
  };
}

/* ---------- 修仙 · 洞府 ---------- */

function workshop(c, tier) {
  const resources = c.resources || {};
  const bag = resources.bag || {};
  const cap = resources.cap || 0;
  const buildings = L.BUILDINGS.map(([key, label, effect]) => {
    const building = resources[key] || {};
    const cost = building.upgrade_cost;
    const parts = [];
    if (cost) {
      if (cost.stones) parts.push(`${L.num(cost.stones)} 灵石`);
      L.RESOURCES.forEach(([res, resLabel]) => {
        const amount = (cost.resources && cost.resources[res]) || 0;
        if (amount) parts.push(`${resLabel} ${L.num(amount)}`);
      });
    }
    return {
      key,
      label,
      effect,
      level: building.level || 0,
      costText: cost ? parts.join(' · ') || '无需材料' : '已满级',
      capped: !!building.capped,
    };
  });
  const skills = [
    ['woodcut', '伐木', resources.woodcut],
    ['stonecut', '采石', resources.stonecut],
  ].map(([key, label, skill]) => {
    const xp = (skill && skill.xp) || 0;
    const need = (skill && skill.xp_to_next) || 0;
    return {
      key,
      label,
      level: (skill && skill.level) || 0,
      xpText: `${L.num(xp)}/${L.num(need)}`,
      percent: bar(need ? xp / need : 0),
    };
  });
  const running = !!c.task;
  const selectedTier = tier || 'common';
  const live = liveProgress(c, Date.now());
  const task = {
    running,
    text: taskText(c),
    idleText: '当前没有进行中的任务',
    stopText: '停止',
    // 卡片头那句话跟着真正跑着的东西走：只有战斗在打时，说「没有进行中的
    // 任务」会和下面那条战斗进度条自相矛盾。
    headHint: running ? '正在进行，可随时停止' : live.battle != null ? '战斗进行中' : '当前没有进行中的任务',
    // 实时进度条。`*Bar` 决定画不画，`*Percent` 是 0..100 的宽度。
    live: live.task != null || live.battle != null,
    taskBar: live.task != null,
    battleBar: live.battle != null,
    taskPercent: bar((live.task || 0) / 1000),
    battlePercent: bar((live.battle || 0) / 1000),
    tiers: L.TIER_CHOICES.map(([key, label]) => ({ key, label, active: key === selectedTier })),
    starts: [
      { act: 'cult-meditate', label: L.TASK_KINDS.Meditate, disabled: running },
      { act: 'cult-woodcut', label: L.TASK_KINDS.Woodcut, disabled: running },
      { act: 'cult-stonecut', label: L.TASK_KINDS.Stonecut, disabled: running },
    ],
  };
  const seclusion = c.seclusion
    ? {
        running: true,
        text: `${L.SECLUSION.title}进行中 · ${L.num(c.seclusion.ticks_done)}/${SECLUSION_TICKS} 回合`,
        hint: L.SECLUSION.hint,
        startText: L.SECLUSION.start,
        stopText: L.SECLUSION.stop,
      }
    : {
        running: false,
        text: L.SECLUSION.title,
        hint: L.SECLUSION.hint,
        startText: L.SECLUSION.start,
        stopText: L.SECLUSION.stop,
      };
  return {
    cap: L.num(cap),
    bag: L.RESOURCES.map(([key, label]) => ({ key, label, value: L.num(bag[key] || 0) })),
    skills,
    buildings,
    task,
    seclusion,
    collectNotice: c.last_collect ? collectText(c.last_collect) : '',
  };
}

/* ---------- 修仙 · 灵草园 ---------- */

function garden(c) {
  const now = Date.now() / 1000;
  const plots = (c.plots || []).map((plot, index) => {
    const crop = plot.crop;
    if (!crop) {
      return { key: `plot-${index}`, index, title: `第 ${index + 1} 块地`, sub: '空地，可以种下', empty: true, canHarvest: false };
    }
    const name = crop.name || (crop.herb ? L.herbText(crop.herb) : '灵草');
    const ready = !crop.withered && crop.ready_at_unix <= now;
    return {
      key: `plot-${index}`,
      index,
      title: `第 ${index + 1} 块地`,
      sub: `${name} · ${L.gradeText(crop.grade)}`,
      state: crop.withered ? '枯萎' : ready ? '可以收获' : `${L.until(crop.ready_at_unix)}后成熟`,
      withered: !!crop.withered,
      ready,
      empty: false,
      canHarvest: ready,
    };
  });

  const seeds = (c.seeds || []).map((seed, index) => {
    const herb = seed.herb || '';
    const name = seed.name || (herb ? L.herbText(herb) : '灵草种子');
    const attr = herb ? '' : ` · ${L.attributeText(seed.attribute)}`;
    return {
      key: `${herb || seed.attribute}-${seed.tier}-${seed.grade}-${index}`,
      index,
      name,
      tierText: `${seed.tier} 阶${attr}`,
      grade: L.gradeText(seed.grade),
      count: seed.count,
      sellPrice: L.num(seed.sell_price),
      herb,
      attribute: seed.attribute,
      tier: seed.tier,
      gradeRaw: seed.grade,
    };
  });

  return {
    rank: c.garden_rank || 1,
    xpText: `${L.num(c.garden_xp)}/${L.num(c.garden_xp_needed)}`,
    percent: bar(c.garden_xp_needed ? c.garden_xp / c.garden_xp_needed : 0),
    plotCount: (c.plots || []).length,
    anyReady: (c.plots || []).some((plot) => plot.crop && !plot.crop.withered && plot.crop.ready_at_unix <= now),
    plots,
    buyPlotText: c.next_plot_price ? `买地 ${L.num(c.next_plot_price)} 灵石` : '地已开满',
    canBuyPlot: !!c.next_plot_price,
    upgradeText: c.garden_upgrade_cost ? `升园阶 ${L.num(c.garden_upgrade_cost)} 灵石` : '园阶已到顶',
    canUpgrade: !!c.garden_upgrade_cost,
    seeds,
  };
}

/* ---------- 修仙 · 集市 ---------- */

function market(c) {
  const offers = (c.shop_offers || []).map((offer) => {
    const line = offer.offer || {};
    let title = '未知货物';
    let sub = '';
    if (line.Equipment) {
      const item = line.Equipment;
      const slot = L.SLOTS.find(([key]) => key === item.slot);
      title = `${item.name || '装备'} · ${L.rarityText(item.rarity)}`;
      sub = `${slot ? slot[1] : item.slot} · ${L.bonusLine(item.attributes, item.bonus)}`;
    } else if (line.Seed) {
      const seed = line.Seed;
      title = seed.name || `${L.attributeText(seed.attribute)}灵草种子`;
      sub = `${seed.tier} 阶 · ${L.attributeText(seed.attribute)} · ${L.gradeText(seed.grade)}`;
    } else if (line.Herb) {
      title = `${L.herbText(line.Herb.herb)} 种子`;
      sub = L.gradeText(line.Herb.grade);
    } else if (line.Art) {
      title = '功法秘本';
      sub = '买下才知道品阶';
    } else if (typeof line.Stones === 'number') {
      title = `${L.num(line.Stones)} 灵石`;
      sub = '一袋灵石';
    }
    const affordable = offer.price <= (c.stones || 0);
    return {
      slot: offer.slot,
      title,
      sub,
      price: L.num(offer.price),
      discount: offer.discount_permille > 0 ? `${(offer.discount_permille / 100).toFixed(1)} 折` : '',
      bought: !!offer.bought,
      disabled: !!offer.bought || !affordable,
      actionText: offer.bought ? '已售' : affordable ? '买下' : '灵石不足',
    };
  });
  return { offers, empty: offers.length === 0 };
}

/* ---------- 功法 · 状态 ---------- */

function statusSection(c) {
  const growthSpan = (c.next || c.growth) - c.floor;
  const guardian = c.guardian;
  const breakdown = c.power_breakdown || {};
  return {
    realm: L.realmText(c.realm),
    sub: `第 ${(c.layer || 0) + 1} 层 · 第 ${c.rank || 1} 阶 · 等级 ${c.level}/${c.total_levels}`,
    power: L.num(c.power),
    external: breakdown.external ? L.num(breakdown.external) : '',
    growthPercent: bar(growthSpan > 0 ? (c.growth - c.floor) / growthSpan : 0),
    growthText: `成长 ${L.num(c.growth)}`,
    nextText: c.next ? `下一级 ${L.num(c.next)}` : c.realm_full ? '本境界已满' : '已至顶',
    stones: L.num(c.stones),
    aptitude: (c.aptitude_tenths / 10).toFixed(1),
    nextGrowth: L.num(c.next_session_growth),
    atHome: !!c.at_home,
    taskText: taskText(c),
    guardian: guardian
      ? {
          target: L.realmText(guardian.target),
          power: L.num(guardian.power),
          mine: L.num(c.power),
          ready: !!c.breakthrough_ready,
          actionText:
            c.power >= guardian.power ? '突破' : `差 ${L.num(guardian.power - c.power)} 战力`,
        }
      : null,
    cooldownText: c.breakthrough_ready_at ? `突破冷却，还需 ${L.until(c.breakthrough_ready_at)}` : '',
    refusalText: c.last_refusal ? L.refusalText(c.last_refusal) : '',
    breakdownText: [
      ['等级', breakdown.level],
      ['加点', breakdown.allocated],
      ['装备属性', breakdown.equipment],
      ['装备加成', breakdown.equipment_bonus],
      ['灵草', breakdown.crop],
      ['内功属性', breakdown.art],
      ['内功加成', breakdown.art_bonus],
    ]
      .map(([label, value]) => `${label} ${L.num(value || 0)}`)
      .join(' · '),
    attributes: L.ATTRIBUTES.map(([key, label]) => {
      const total = (c.attributes && c.attributes[key]) || 0;
      const level = (c.level_attributes && c.level_attributes[key]) || 0;
      const extra = total - level;
      return { key, label, total: L.num(total), extra: extra > 0 ? `+${L.num(extra)}` : '' };
    }),
    derived: L.DERIVED.map(([key, label]) => ({
      key,
      label,
      value: L.num((c.derived && c.derived[key]) || 0),
    })),
    freePoints: c.free_points || 0,
    alloc: L.ATTRIBUTES.map(([key, label]) => ({
      key,
      label,
      allocated: L.num((c.allocated_attributes && c.allocated_attributes[key]) || 0),
      canOne: (c.free_points || 0) > 0,
      canTen: (c.free_points || 0) >= 10,
    })),
    modes: L.MODES.map(([key, label]) => ({ key: key.toLowerCase(), label, active: c.session_mode === key })),
    herbs: [
      { key: 'insight', label: '悟心草', count: (c.aptitude_herbs && c.aptitude_herbs[0]) || 0 },
      { key: 'wisdom', label: '慧心草', count: (c.aptitude_herbs && c.aptitude_herbs[1]) || 0 },
      { key: 'marrow', label: '洗髓丹', count: (c.aptitude_herbs && c.aptitude_herbs[2]) || 0 },
    ],
    marrowCost: c.marrow_gate_cost || 0,
    dailyEvents: c.daily_events || 0,
    dailyEventDay: c.daily_event_day || 0,
    hasDailyEvent: !!c.last_daily_event,
  };
}

/* ---------- 功法 · 功法 ---------- */

function artsSection(c) {
  const arts = (c.arts || []).map((art) => ({
    id: art.id,
    name: art.name,
    grade: L.gradeText(art.grade),
    kind: art.kind === 'Internal' ? '内功' : '外功',
    rank: `${art.rank}/${art.max_rank}`,
    selected: !!art.selected,
    progressText: `${L.num(art.progress)}/${L.num(art.required)}`,
    percent: bar(art.required ? art.progress / art.required : 0),
    rate: (art.success_permille / 10).toFixed(1),
    copies: art.copies > 1 ? `重复 ${art.copies} 本` : '',
  }));
  const mastered = (c.mastered_arts || []).map((art, index) => ({
    key: `${art.name}-${index}`,
    name: art.name,
    kind: art.kind === 'Internal' ? '内功' : '外功',
    line: L.bonusLine(null, art.bonus),
  }));
  const artAttributes = c.art_attributes || {};
  return {
    arts,
    mastered,
    summary: `内功属性 ${L.num(artAttributes.root || 0)}/${L.num(artAttributes.strength || 0)}/${L.num(
      artAttributes.mana || 0,
    )}/${L.num(artAttributes.agility || 0)} · 折合战力 ${L.num(c.art_power)}`,
    external: c.external_power ? `外功每击 +${L.num(c.external_power)}` : '',
    sets: (c.art_sets || []).map((set, index) => ({
      key: `art-set-${index}`,
      text: `同门 ${set.books} 本 · ${L.bonusLine(null, set.bonus)}`,
    })),
  };
}

/* ---------- 功法 · 装备 ---------- */

function gearSection(c) {
  const equipped = c.equipped || [];
  return {
    slots: L.SLOTS.map(([key, label], index) => {
      const item = equipped[index];
      if (!item) return { key, label, empty: true };
      return {
        key,
        label,
        empty: false,
        name: item.name || '',
        rarity: L.rarityText(item.rarity),
        star: item.star ? '★'.repeat(item.star) : '',
        line: L.bonusLine(item.attributes, item.bonus),
      };
    }),
    inventory: (c.inventory || []).map((item) => ({
      id: item.id,
      name: item.name || '',
      rarity: L.rarityText(item.rarity),
      tierText: `${item.tier} 阶`,
      star: item.star ? `· ${'★'.repeat(item.star)}` : '',
      set: item.set ? `· 套装 ${item.set.name || ''}` : '',
      line: L.bonusLine(item.attributes, item.bonus),
      slot: item.slot,
      starCost: item.star_cost ? L.num(item.star_cost.stones) : '',
      canStar: !!item.star_cost,
      sellPrice: L.num(item.sell_price),
    })),
    count: (c.inventory || []).length,
    sets: (c.gear_sets || []).map((set, index) => ({
      key: `gear-set-${index}`,
      text: `${(set.set && set.set.name) || '套装'} ${set.pieces} 件 · ${L.bonusLine(null, set.bonus)}${set.effect ? ` · 完整 ${SET_EFFECTS[set.effect] || set.effect}` : ''}`,
    })),
  };
}

/* ---------- 我的 ---------- */

function me(snapshot, profile) {
  const c = (snapshot && snapshot.cultivation) || {};
  const nickname = (profile && profile.nickname) || '未命名道友';
  return {
    nickname,
    avatar: (profile && profile.avatar) || '',
    initial: nickname.slice(0, 1),
    realm: L.realmText(c.realm),
    level: c.level || 1,
    layer: (c.layer || 0) + 1,
    rank: c.rank || 1,
    power: L.num(c.power),
    stones: L.num(c.stones),
    pomodoros: snapshot ? snapshot.total_pomodoros || 0 : 0,
    focusMinutes: snapshot ? snapshot.focus_minutes || 25 : 25,
    expeditions: c.explorations || 0,
    provings: c.provings || 0,
    floor: c.trial_floor || 0,
    arts: (c.arts || []).length,
    mastered: (c.mastered_arts || []).length,
    gear: (c.inventory || []).length,
    plots: (c.plots || []).length,
    events: c.daily_events || 0,
  };
}

module.exports = {
  artsSection,
  bar,
  calendarView,
  garden,
  gearSection,
  header,
  liveProgress,
  market,
  me,
  outing,
  pomodoro,
  statusSection,
  todayCount,
  tower,
  workshop,
};
