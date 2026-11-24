// The cultivation ladder: every page under 修为.
//
// All numbers come from the wasm snapshot, which is the desktop's own
// `CultivationView` serialized — so this file renders, it never computes.

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

const SET_EFFECTS = {
  frost: '寒霜',
  thunder: '雷亟',
  sunder: '碎甲',
  weaken: '蛊蚀',
};

const ATTRIBUTES = [
  ['root', '根骨'],
  ['strength', '力道'],
  ['mana', '灵力'],
  ['agility', '身法'],
];

const DERIVED = [
  ['attack', '攻击'],
  ['health', '气血'],
  ['inner', '内功'],
  ['spell', '法术'],
  ['resist', '抗性'],
  ['speed', '速度'],
  ['hit', '命中'],
  ['evade', '闪避'],
];

const DERIVED_NAMES = {
  Attack: '攻击',
  Health: '气血',
  Inner: '内功',
  Spell: '法术',
  Resist: '抗性',
  Speed: '速度',
  Hit: '命中',
  Evade: '闪避',
  Aptitude: '潜质',
};

const SLOTS = [
  ['weapon', '武器'],
  ['clothes', '衣袍'],
  ['pants', '裤装'],
  ['boots', '鞋靴'],
];

const RARITY = {
  common: ['凡品', 'rarity-common'],
  fine: ['良品', 'rarity-fine'],
  rare: ['珍品', 'rarity-rare'],
  epic: ['奇珍', 'rarity-epic'],
  legendary: ['神品', 'rarity-legendary'],
};

const GRADE = {
  Yellow: ['黄阶', 'grade-yellow'],
  Profound: ['玄阶', 'grade-profound'],
  Earth: ['地阶', 'grade-earth'],
  Heaven: ['天阶', 'grade-heaven'],
  Divine: ['神阶', 'grade-divine'],
  yellow: ['黄阶', 'grade-yellow'],
  profound: ['玄阶', 'grade-profound'],
  earth: ['地阶', 'grade-earth'],
  heaven: ['天阶', 'grade-heaven'],
  divine: ['神阶', 'grade-divine'],
};

const HERBS = {
  Insight: '悟心草',
  Wisdom: '慧心草',
  Marrow: '洗髓丹',
  insight: '悟心草',
  wisdom: '慧心草',
  marrow: '洗髓丹',
};

const SITES = { Forest: '森林', Mountain: '山麓', Mine: '矿洞', forest: '森林', mountain: '山麓', mine: '矿洞' };
const BUILDINGS = {
  Workshop: '工坊',
  Warehouse: '仓库',
  Smithy: '铁匠铺',
  Dojo: '道场',
  workshop: '工坊',
  warehouse: '仓库',
  smithy: '铁匠铺',
  dojo: '道场',
};
const MODES = { Study: '修习', Woodcut: '伐木', Mine: '采石', study: '修习', woodcut: '伐木', mine: '采石' };
// 实时任务的叫法，快照里的 `task`/`seclusion`/`battle` 见 `CultivationView`。
// 吐纳已下线（`docs/0103` §4），它不再出现在快照里。
const TASK_KINDS = { Woodcut: '伐木', Stonecut: '采石', Work: '打工' };
// 采集深度：一颗数字加一个倍率，不再是第二套稀有度词（`docs/0105`）。
// 深度与倍率由快照给：`resources.woodcut.depth` / `.depth_base_permille`、
// `rules.gather_depths` 都是内核算好的，页面不再留等级表（`docs/0106`）。
const GATHER_TIER_NUMERALS = { Common: 'Ⅰ', Spirit: 'Ⅱ', Divine: 'Ⅲ', Chaos: 'Ⅳ' };
// 同一个采集点越走越深的名字：森林那条线是凡木林→混沌神木，山那条是凡石山→星辰矿脉。
const GATHER_PLACES = {
  Forest: { Common: '凡木林', Spirit: '灵木林', Divine: '神木林', Chaos: '混沌神木' },
  Mountain: { Common: '凡石山', Spirit: '铁矿山', Divine: '寒铁窟', Chaos: '星辰矿脉' },
};

// 一个深度自己的倍率，读内核的 `rules.gather_depths`（千分比）。
function depthMultiplier(c, tier) {
  const row = (c.rules?.gather_depths ?? []).find((entry) => entry.tier === tier);
  return ((row?.base_permille ?? 0) / 1000).toFixed(1);
}
function depthTag(c, tier) {
  return `${GATHER_TIER_NUMERALS[tier] ?? tier} · ×${depthMultiplier(c, tier)}`;
}
const JOB_GRADE_NAMES = ['黄阶', '玄阶', '地阶', '天阶', '星阶'];
const BATTLE_KINDS = { Trial: '锁妖塔之战', Proving: '试炼之战', Breakthrough: '破境之战' };
// 闭关一次多少回合、多少秒，都由 `rules` 给（内核 `SECLUSION_TICKS` /
// `SECLUSION_TICK_SECONDS`），页面不再写死 250。
const REGIONS = [
  ['azure_peaks', '青云山脉', '低阶妖兽盘踞的山脉，适合初入江湖的修士。'],
  ['frozen_wastes', '玄冰荒漠', '千里冰封的荒芜之地，寒属性的法宝与药草藏于其间。'],
  ['flame_caverns', '赤炎洞窟', '地火翻涌的炽热洞窟，锻造与火系功法的宝地。'],
  ['shadow_realm', '幽冥鬼域', '鬼气弥漫的幽冥之地，诡异的法宝与阴属性材料在此诞生。'],
  ['celestial_court', '天外仙境', '仙灵之气充盈的至高秘境，真正的仙家材料与神器现世。'],
  ['starfall_plains', '星陨古原', '陨星坠落形成的荒原，星屑与虚空灵石埋于裂谷之下。'],
  ['blood_sea', '九幽血海', '血色浪潮翻涌的地渊，魔道残魂与血煞法宝在此沉浮。'],
  ['void_sanctum', '太虚秘境', '虚实交错的秘境，法则在此松动，太虚之物偶尔流入人间。'],
  ['thunder_abyss', '雷罚天渊', '劫雷不断劈落的天渊，雷劫炼成的法宝与雷属性灵药满地。'],
  ['chaos_palace', '混沌天宫', '开天辟地之前的宫殿，混沌之气凝成的至宝仍在其中沉睡。'],
];

const RESOURCES = [
  ['wood', '木'],
  ['stone', '石'],
  ['iron', '铁'],
  ['coal', '煤'],
];

// 采集通知里采集点的叫法，与 App 的 `cultivation.site.*` 逐字一致。上面的
// `SITES` 是任务行用的简称（森林/山麓/矿洞），两者刻意分开。
const COLLECT_SITES = { Forest: '森林·伐木', Mountain: '山·采石', Mine: '矿洞·双技能' };

// 一袋资源的一句话：「木×4 · 煤×2」。空的袋子说「空手而归」，不画一个空串。
function bagText(bag) {
  if (!bag) return '';
  return RESOURCES.filter(([key]) => (bag[key] ?? 0) > 0)
    .map(([key, label]) => `${label}×${quantity(bag[key])}`)
    .join(' · ');
}

const FIGHTS = {
  Guardian: '境界守关者',
  Wanderer: '游历散修',
  Trial: '塔中对手',
  guardian: '境界守关者',
  wanderer: '游历散修',
  trial: '塔中对手',
};

const REFUSALS = {
  ExploreCooldown: '还在回来的路上',
  NotOut: '人不在外面',
  ExpeditionOut: '正在外出采集',
  TrialOut: '人不在家，进不了塔',
  Working: '正在打工',
  JobGone: '这份活已经没了',
  TrialDrained: '今日塔次已用完',
  TrialTop: '已经到塔顶',
  TrialNotSweepable: '守关者还不够弱，无法横扫',
  ProvingSpent: '没有试炼次数了',
  ProvingOut: '人不在家',
  RegionLocked: '该区域尚未开启：先提升境界',
  TaskRunning: '已有任务进行中，先停掉再开始',
  SeclusionRunning: '正在闭关，先坐完这一回',
  BattleRunning: '已有一场战斗在进行，先等它打完',
  BreakthroughCooling: '破境尚在调息，稍后再来',
  BattleUnavailable: '这一场现在还无法开始',
  Out: '人不在家',
  Working: '正在打工',
  Spent: '次数已用完',
  Drained: '今日次数已用完',
  Top: '已经到顶',
};

export const CULTIVATION_TABS = [
  { id: 'status', label: '状态' },
  { id: 'record', label: '记录' },
  { id: 'gear', label: '装备' },
  { id: 'arts', label: '功法' },
  { id: 'garden', label: '灵草园' },
  { id: 'market', label: '集市' },
  { id: 'outing', label: '外出' },
  { id: 'tower', label: '塔与试炼' },
  { id: 'workshop', label: '洞府' },
];

const esc = (value) =>
  String(value ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);

const num = (value) => Number(value ?? 0).toLocaleString('zh-CN');

// 资源与采集经验：一万以下给完整数字并加千位符（9,999），再大用 K（12.3K），
// 百万以上用 M（1.23M）。与内核 `quantity_number`（`bongocat-i18n`）同一把尺子：
// 进位在单位自己的小数位里做，所以 999,999 读作 1M 而不是 1000K。
const QUANTITY_UNITS = [
  [1e12, 'T', 2],
  [1e9, 'B', 2],
  [1e6, 'M', 2],
  [1e3, 'K', 1],
];

export function quantity(value) {
  const raw = Math.round(Number(value ?? 0));
  if (!Number.isFinite(raw)) return '0';
  const sign = raw < 0 ? '-' : '';
  const magnitude = Math.abs(raw);
  if (magnitude < 10_000) return num(raw);
  for (const [scale, suffix, places] of QUANTITY_UNITS) {
    const factor = 10 ** places;
    const scaled = Math.round((magnitude * factor) / scale);
    if (scaled >= factor && scaled < 10_000 * factor) {
      const whole = Math.floor(scaled / factor);
      const fraction = String(scaled % factor)
        .padStart(places, '0')
        .replace(/0+$/, '');
      return `${sign}${whole}${fraction ? `.${fraction}` : ''}${suffix}`;
    }
  }
  return `${sign}${Math.floor(magnitude / 1e12)}T`;
}

function grade(value) {
  const [text, cls] = GRADE[value] ?? ['黄阶', 'grade-yellow'];
  return `<span class="${cls}">${text}</span>`;
}

function rarity(value) {
  const [text, cls] = RARITY[value] ?? ['凡品', 'rarity-common'];
  return `<span class="${cls}">${text}</span>`;
}

function herbName(value) {
  return HERBS[value] ?? String(value ?? '');
}

function duration(seconds) {
  const total = Math.max(0, Math.round(seconds));
  if (total < 60) return `${total} 秒`;
  const minutes = Math.floor(total / 60);
  if (minutes < 60) return `${minutes} 分`;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  if (hours < 24) return rest ? `${hours} 时 ${rest} 分` : `${hours} 时`;
  const days = Math.floor(hours / 24);
  return `${days} 天 ${hours % 24} 时`;
}

function until(unix) {
  const now = Date.now() / 1000;
  return duration(unix - now);
}

function when(unix) {
  const date = new Date(unix * 1000);
  const pad = (value) => String(value).padStart(2, '0');
  return `${date.getMonth() + 1} 月 ${date.getDate()} 日 ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function refusalText(refusal) {
  if (!refusal) return null;
  const key = typeof refusal === 'string' ? refusal : Object.keys(refusal)[0];
  if (key === 'Trade') {
    const detail = refusal.Trade;
    return {
      NotOwned: '手里没有这件东西',
      NotEnoughStones: '灵石不够',
      NotEnoughResources: '材料不够',
      NoSecondaryAttribute: '这件装备只有一条属性，无法重铸',
    }[detail] ?? '交易没有成立';
  }
  return REFUSALS[key] ?? '条件不足';
}

/* ---------- 通知与事件 ----------
 *
 * 口径与 App 的 `cultivation.notice.*` / `cultivation.event.*` 逐字一致
 * （`docs/0106`）：通知是「上一次动作的结果」，事件是「按时间排的历史」。
 * 两者都不在这里算数，只把快照里的字段翻成人话；游戏规则仍在内核。
 */

const ATTRIBUTE_NAMES = Object.fromEntries(ATTRIBUTES.map(([key, label]) => [key, label]));
const REALM_ORDER = Object.keys(REALMS);
const BUILDING_NAMES = { Workshop: '工坊', Warehouse: '仓库', Smithy: '铁匠铺', Dojo: '道场' };
const MARROW_SOURCES = { Combat: '战斗', Fortune: '机缘', Shop: '集市' };
const WALK_OUTCOMES = {
  Nothing: '一无所获。',
  Seed: '寻得仙草',
  Gear: '寻得装备',
  Art: '寻得功法',
  Stones: '寻得灵石',
  Victory: '遭遇战胜利，小有斩获。',
  Defeat: '遭遇战败北，幸而未失。',
};
const WALK_HINTS = {
  walk_hint_slow: '进度非常缓慢',
  walk_hint_slow_alt: '举步维艰',
  walk_hint_slow_alt2: '收效甚微',
  walk_hint_uneven: '稍有波折',
  walk_hint_uneven_alt: '小有阻碍',
  walk_hint_uneven_alt2: '略显坎坷',
  walk_hint_steady: '稳步推进',
  walk_hint_steady_alt: '循序渐进',
  walk_hint_steady_alt2: '扎实前行',
  walk_hint_fast: '突飞猛进',
  walk_hint_fast_alt: '一日千里',
  walk_hint_fast_alt2: '势如破竹',
};

const gradeName = (value) => (GRADE[value] ?? ['黄阶'])[0];
const attributeName = (key) => ATTRIBUTE_NAMES[key] ?? key ?? '';
const herbNameOf = (key) => HERBS[key] ?? key ?? '';
const realmName = (index) => REALM_ORDER[index] ? REALMS[REALM_ORDER[index]] : `第 ${index} 境`;
const siteName = (site) => COLLECT_SITES[site] ?? site ?? '';

// 一株种子/一畦作物：有名字用名字，没有名字用「属性种子 · 阶 · 品阶」。
function seedText(name, attribute, tier, gradeValue) {
  const gradeLabel = gradeName(gradeValue);
  if (name) return `${name} · ${tier} 阶 · ${gradeLabel}`;
  return `${attributeName(attribute)}种子 · ${tier} 阶 · ${gradeLabel}`;
}

function herbSeedText(herb, tier, gradeValue) {
  return `${herbNameOf(herb)}种子 · ${tier} 阶 · ${gradeName(gradeValue)}`;
}

function audienceName(side) {
  return side === 'Player' ? '你' : side === 'Opponent' ? '对手' : String(side ?? '');
}

// 一袋资源的一句话：「木×4 · 煤×2」；空袋给空串，让调用方决定怎么收尾。
function resourceText(bag) {
  return bagText(bag);
}

function hintText(key) {
  if (!key) return '';
  return WALK_HINTS[String(key).split('.').pop()] ?? '';
}

// 一次升星/建造的代价，与 App 的 `%{stones} 灵石、%{tribute}` 同序。
function costText(cost) {
  if (!cost) return '';
  const stones = num(cost.stones ?? 0);
  const tribute = resourceText(cost.resources);
  return tribute ? `${stones} 灵石、${tribute}` : `${stones} 灵石`;
}

function saleName(what) {
  if (!what) return '东西';
  if (typeof what === 'string') return what === 'Plot' ? '灵田' : '东西';
  const [tag, body] = Object.entries(what)[0] ?? [];
  if (tag === 'Equipment') return body?.name ?? '装备';
  if (tag === 'Seed') return seedText(body?.name, body?.attribute, body?.tier, body?.grade);
  if (tag === 'Art') return body?.name ?? '功法';
  if (tag === 'Plot') return '灵田';
  if (tag === 'Herb') return herbNameOf(body?.herb);
  return '东西';
}

function bonusFindText(find) {
  if (!find) return '';
  const [tag, body] = Object.entries(find)[0] ?? [];
  if (tag === 'Equipment') return body?.name ?? '装备';
  if (tag === 'Seed') return seedText(null, body?.attribute, body?.tier, body?.grade);
  if (tag === 'Art') return body?.name ?? '功法';
  if (tag === 'Herb') return herbNameOf(body?.herb);
  if (tag === 'Stones') return `${num(body?.stones ?? 0)} 灵石`;
  return '';
}

function foundSegments(art, gear, seed) {
  const parts = [];
  if (art) parts.push(`功法「${esc(art)}」`);
  if (gear) parts.push(`装备「${esc(gear)}」`);
  if (seed) parts.push(`仙草「${esc(seed)}」`);
  return parts;
}

function walkLine(body) {
  const parts = [`外出探索：${WALK_OUTCOMES[body.outcome] ?? body.outcome ?? ''}`];
  parts.push(...foundSegments(body.art, body.gear, body.seed));
  if (body.stones != null) parts.push(`灵石「${num(body.stones)}」`);
  if (body.gain > 0) parts.push(`探索 +${(body.gain / 10).toFixed(1)}%`);
  const hint = hintText(body.hint);
  if (hint) parts.push(hint);
  return parts.join(' · ');
}

function trialLine(body) {
  const parts = [body.won ? `第 ${body.floor} 层通关。` : `第 ${body.floor} 层未过。`];
  if (body.won) {
    parts.push(...foundSegments(body.art, body.gear, body.seed));
    if (body.stones != null) parts.push(`灵石「${num(body.stones)}」`);
  }
  return parts.join(' · ');
}

function sweepLine(body) {
  const from = body.from_floor ?? 0;
  const to = from + Math.max(0, (body.floors ?? 0) - 1);
  const parts = [`横扫第 ${from}~${to} 层`];
  if (body.seeds > 0) parts.push(`仙草 ×${body.seeds}`);
  if (body.gear > 0) parts.push(`装备 ×${body.gear}`);
  if (body.arts > 0) parts.push(`功法 ×${body.arts}`);
  if (body.stones > 0) parts.push(`灵石「${num(body.stones)}」`);
  return parts.join(' · ');
}

// 一条事件，按 App `records.rs` 的 `event_line` 同一套模板读出来。
// 未知变体不猜内容：给一个中性的「有新的动静」，免得把结构化字段直接摊给用户。
export function eventLine(detail) {
  if (!detail) return '';
  if (typeof detail === 'string') return detail === 'Harvest' ? '仙草园收成了一株。' : '';
  const [tag, body = {}] = Object.entries(detail)[0] ?? [];
  switch (tag) {
    case 'Session':
      return `完成修炼，成长 +${num(body.growth)}`;
    case 'SessionGather': {
      const parts = [`采集完一个番茄钟：${siteName(body.site)} 得 ${resourceText(body.gained) || '空手而归'}`];
      if (body.woodcut_xp > 0) parts.push(`经验 +${num(body.woodcut_xp)}`);
      if (body.stonecut_xp > 0) parts.push(`经验 +${num(body.stonecut_xp)}`);
      return parts.join('，');
    }
    case 'Expedition': {
      const parts = [`外出采集归来：${siteName(body.site)} ${body.hours} 小时，得 ${resourceText(body.gained) || '空手而归'}`];
      if (body.woodcut_xp > 0) parts.push(`经验 +${num(body.woodcut_xp)}`);
      if (body.stonecut_xp > 0) parts.push(`经验 +${num(body.stonecut_xp)}`);
      return parts.join('，');
    }
    case 'Found': {
      const parts = ['番茄钟掉落'];
      parts.push(...foundSegments(null, body.gear, body.seed));
      return parts.join(' · ');
    }
    case 'Work': {
      const parts = [
        `打工 ${body.hours} 小时，得 ${num(body.stones)} 灵石，达成率 ${Math.floor((body.success_rate ?? 0) / 10)}%`,
      ];
      if (body.gain) {
        const [slot, points] = body.gain;
        parts[0] += `，额外 +${num(points)} ${DERIVED_NAMES[slot] ?? slot}`;
      }
      return parts.join('');
    }
    case 'Absence':
      return `${body.days} 天未修炼，成长值 -${num(body.charged)}`;
    case 'Walk':
      return walkLine(body);
    case 'RegionCleared':
      return `首次探明 ${REGIONS[body.region]?.[1] ?? `第 ${body.region + 1} 区`} · 灵石 +${num(body.stones)} · ${body.gear ?? ''}`;
    case 'Aptitude': {
      const line = `${herbNameOf(body.herb)} · 潜质 +${((body.gained_tenths ?? 0) / 10).toFixed(1)} → ${((body.value_tenths ?? 0) / 10).toFixed(1)}`;
      return body.marrow > 0 ? `${line}（耗洗髓丹 ${body.marrow}）` : line;
    }
    case 'MarrowFound':
      return `得洗髓丹 ×1（${MARROW_SOURCES[body.source] ?? body.source}）`;
    case 'Training': {
      const key = {
        Failed: `修习《${body.name}》未成，进度仍在`,
        Progressed: `修习《${body.name}》有所精进`,
        RankedUp: `《${body.name}》突破至第 ${body.rank} 阶`,
        Mastered: `《${body.name}》已至圆满`,
      }[body.outcome];
      return key ?? `修习《${body.name}》`;
    }
    case 'TrainBody':
      return `练功有成，属性点 +${num(body.points)}`;
    case 'Breakthrough': {
      const parts = [];
      if (body.won) {
        parts.push(
          body.stage != null ? `破境成功，晋入${realmName(body.stage)}。` : '闯关成功，晋入新境界！',
        );
        parts.push(...foundSegments(body.art, body.gear, body.seed));
      } else {
        parts.push('闯关未成，尚需砥砺。');
      }
      if (body.stones != null) parts.push(`灵石「${num(body.stones)}」`);
      return parts.join(' · ');
    }
    case 'Trial':
      return trialLine(body);
    case 'TrialSweep':
      return sweepLine(body);
    case 'Proving': {
      if (!body.won) return '试炼落败，未有所获。';
      const parts = ['试炼获胜。'];
      parts.push(...foundSegments(body.art, body.gear, body.seed));
      if (body.stones != null) parts.push(`灵石「${num(body.stones)}」`);
      if (body.progress != null) parts.push(`试炼 +${(body.progress / 10).toFixed(1)}%`);
      return parts.join(' · ');
    }
    case 'Bought':
      return body.stones > 0
        ? `花 ${num(body.stones)} 灵石购入 ${saleName(body.what)}`
        : `购入 ${saleName(body.what)}`;
    case 'Sold':
      return `售出 ${saleName(body.what)}，得 ${num(body.stones)} 灵石`;
    case 'Harvest':
      return '仙草园收成了一株。';
    case 'Harvested': {
      let line = body.herb
        ? `仙草园收成 ${herbSeedText(body.herb, body.tier, body.grade)}（潜质灵草）`
        : `仙草园收成 ${seedText(body.name, body.attribute, body.tier, body.grade)}，${attributeName(body.attribute)} +${num(body.points)}`;
      if (body.xp > 0) line += ` · 造化 +${num(body.xp)}`;
      if (body.seed) line += ` · 收获返得 ${esc(body.seed)}`;
      return line;
    }
    case 'Withered': {
      const seed = body.herb
        ? herbSeedText(body.herb, body.tier, body.grade)
        : seedText(body.name, body.attribute, body.tier, body.grade);
      return `${seed} 未能成活（${((body.permille ?? 0) / 10).toFixed(0)}%）· 造化 +${num(body.xp)}`;
    }
    case 'GardenRank':
      return `仙草园晋至 ${body.rank} 阶（花 ${num(body.stones)} 灵石）`;
    case 'Daily': {
      const key =
        body.kind === 'Ambush'
          ? body.won
            ? '晨间遇袭，击退来犯'
            : '晨间遇袭，不敌而退，所幸未失'
          : body.kind === 'Gift'
            ? '晨起有得'
            : '晨间无事，拾得零碎';
      const parts = [key];
      parts.push(...foundSegments(body.art, body.gear, body.seed));
      if (body.stones != null) parts.push(`灵石「${num(body.stones)}」`);
      return parts.join(' · ');
    }
    case 'Star': {
      const stars = '★'.repeat(body.star ?? 0);
      return body.success
        ? `${esc(body.name)} 升星成功，${stars}（花 ${num(body.cost?.stones)} 灵石、${resourceText(body.cost?.resources) || '无'}）`
        : `${esc(body.name)} 升星失败，降至 ${body.star} 星（花 ${num(body.cost?.stones)} 灵石、${resourceText(body.cost?.resources) || '无'}）`;
    }
    case 'Reforge':
      return `${esc(body.name)} 重铸：${attributeName(body.from)} ${num(body.from_points)} → ${attributeName(body.to)} ${num(body.to_points)}（花 ${num(body.cost?.stones)} 灵石、${resourceText(body.cost?.resources) || '无'}）`;
    case 'BuildingUpgrade':
      return `${BUILDING_NAMES[body.building] ?? body.building} 升至 ${body.level} 级（花 ${num(body.cost?.stones)} 灵石、${resourceText(body.cost?.resources) || '无'}）`;
    case 'Bonus':
      return `意外之财：${bonusFindText(body.find)}`;
    case 'EquipmentLost':
      return `战败掉落装备「${esc(body.name)}」`;
    default:
      return '';
  }
}

function stamp(unix) {
  const date = new Date(Number(unix ?? 0) * 1000);
  const pad = (value) => String(value).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

// 一场战斗的逐回合回放。App 有动画条，这里给同一份回合表的文字版：
// 每一回合「谁、用什么、打掉多少、还剩多少」，与 App 的 `DuelRound` 同字段。
function replayMarkup(duel) {
  if (!duel || !Array.isArray(duel.rounds) || !duel.rounds.length) return '';
  const rows = duel.rounds
    .map((round) => {
      const art = round.art ? ` · ${esc(round.art)}` : '';
      const blow = round.dodged ? '被闪避' : `-${num(round.damage)}`;
      const effect = round.effect ? ` · ${esc(typeof round.effect === 'string' ? round.effect : Object.keys(round.effect)[0])}` : '';
      return `<div class="round"><span class="round-index">${round.index}.</span> ${esc(round.attacker)}${art} ${blow}${effect} <span class="muted">剩 ${num(round.target_health)}</span></div>`;
    })
    .join('');
  const verdict = duel.won ? '胜' : '负';
  return `
    <details class="replay">
      <summary>回放（${duel.rounds.length} 回合 · ${verdict}）</summary>
      <div class="rounds">${rows}</div>
    </details>`;
}

// 通知条：最近一次动作的结果，与 App `notices()` 同一组 key。
export function noticeLines(c) {
  const lines = [];
  if (!c) return lines;
  if (c.last_absence) {
    lines.push({ key: 'absence', tone: 'danger', text: `${c.last_absence.days} 天未修炼，成长值 -${num(c.last_absence.charged)}` });
  }
  if (c.last_drop) {
    const [tag, body = {}] = Object.entries(c.last_drop)[0] ?? [];
    const text = {
      Equipment: `获得装备 ${body.name ?? '装备'}`,
      Seed: `获得种子 ${seedText(null, body.attribute, body.tier, body.grade)}`,
      Art: `千分之一：获得功法《${body.name}》`,
      Herb: `千分之一：获得 ${herbNameOf(body.herb)}`,
      Stones: `千分之一：获得 ${num(body)} 灵石`,
    }[tag];
    if (text) lines.push({ key: 'drop', tone: 'accent', text });
  }
  if (c.last_seed_return) {
    const seed = c.last_seed_return;
    lines.push({
      key: 'seed-return',
      tone: 'accent',
      text: `收获返得 ${seedText(seed.name, seed.attribute, seed.tier, seed.grade)}`,
    });
  }
  if (c.last_star) {
    const star = c.last_star;
    const name = star.name ?? '武器';
    lines.push({
      key: 'star',
      tone: star.success ? 'accent' : 'danger',
      text: star.success ? `升星成功：${name} 升至 ${star.to} 星` : `升星失败：${name} 降至 ${star.to} 星`,
    });
  }
  if (c.last_reforge) {
    const reforge = c.last_reforge;
    lines.push({
      key: 'reforge',
      tone: 'accent',
      text: `重铸 ${esc(reforge.name ?? '装备')}：${attributeName(reforge.from)} ${num(reforge.from_points)} → ${attributeName(reforge.to)} ${num(reforge.to_points)}`,
    });
  }
  if (c.last_collect) {
    const collect = c.last_collect;
    const lost = Object.values(collect.lost ?? {}).some((n) => (n ?? 0) > 0);
    const bag = lost ? resourceText(collect.lost) : resourceText(collect.gained);
    const xp = (collect.woodcut_xp ?? 0) + (collect.stonecut_xp ?? 0);
    lines.push({
      key: 'collect',
      tone: 'accent',
      text: lost
        ? `${siteName(collect.site)}：仓库已满，${bag} 未能入袋，采集经验 +${num(xp)}`
        : `${siteName(collect.site)}：获得 ${bag || '空手而归'}，采集经验 +${num(xp)}`,
    });
  }
  if (c.last_building) {
    const building = c.last_building;
    lines.push({
      key: 'building',
      tone: 'accent',
      text: `${BUILDING_NAMES[building.building] ?? building.building} 升至 ${building.to} 级`,
    });
  }
  return lines;
}

export function noticeStrip(c) {
  const lines = noticeLines(c);
  if (!lines.length) return '';
  return `<div class="notices">${lines
    .map((line) => `<div class="notice ${line.tone}" data-notice="${line.key}">${line.text}</div>`)
    .join('')}</div>`;
}

const VISIBLE_EVENT_COUNT = 5;

// 记录页：在家/外出这一行 + 最近 5 件事，其余折叠在「查看全部」里。
// 每条事件带时间戳；带战斗的事件多一个可展开的逐回合回放。
function recordPage(c) {
  const events = c.events ?? [];
  const returning = c.walk_early || c.work_early;
  const homeAt = c.work?.ends_unix ?? c.exploration_ready_at;
  const status = c.at_home
    ? '在家'
    : returning
      ? `返程中：尚需 ${duration((homeAt ?? 0) - Date.now() / 1000)}到家`
      : c.work
        ? `外出：在打工（${JOB_GRADE_NAMES[c.work.grade] ?? ''}·${c.work.hours} 小时），剩余 ${duration((c.work.ends_unix ?? 0) - Date.now() / 1000)}返回`
        : c.exploration_ready_at
          ? `外出：正在探索，剩余 ${duration(c.exploration_ready_at - Date.now() / 1000)}返回`
          : '外出';
  const trip = c.outing_return_seconds ?? 0;
  const hours = Math.floor(trip / 3600);
  const minutes = Math.max(Math.floor((trip % 3600) / 60), hours === 0 ? 1 : 0);
  const row = (event) => `
    <div class="row">
      <div class="row-main">
        <div class="row-sub">${stamp(event.at_unix)} ${eventLine(event.detail) || '（这一条没有可读的记述）'}</div>
        ${replayMarkup(event.detail?.Walk?.duel ?? event.detail?.Trial?.duel ?? event.detail?.Proving?.duel ?? event.detail?.Breakthrough?.duel ?? event.detail?.Daily?.duel)}
      </div>
    </div>`;
  const head = events.slice(0, VISIBLE_EVENT_COUNT).map(row).join('');
  const rest = events.slice(VISIBLE_EVENT_COUNT).map(row).join('');
  return `
    <div class="card">
      <div class="card-head">
        <div class="card-title">行踪</div>
        <div class="card-hint">${status}</div>
      </div>
      ${
        c.at_home
          ? ''
          : `<div class="form-row">
              <button class="btn small" data-act="cult-return-early" ${returning ? 'disabled' : ''} title="提前返回没有任何奖励：本次外出作废，返程要走上 ${hours} 小时 ${minutes} 分。">
                ${returning ? '返程中' : '提前返回'}
              </button>
            </div>`
      }
    </div>

    <div class="card">
      <div class="card-head">
        <div class="card-title">最近事件</div>
        <div class="card-hint">${events.length} 件</div>
      </div>
      <div class="rows">
        ${head || '<div class="empty">还没有动静。完成番茄钟、外出探索，都记在这里。</div>'}
      </div>
      ${
        rest
          ? `<details class="show-all"><summary>查看全部 ${events.length} 件事</summary><div class="rows">${rest}</div></details>`
          : ''
      }
    </div>
  `;
}

// 今日境界/战力增量，与 App「今日境界 ±N 级 · 战力 ±M」同一读法。
function todayDeltaText(c) {
  const level = c.today_level_delta ?? 0;
  const power = c.today_power_delta ?? 0;
  if (!level && !power) return '今日境界 ±0 级 · 战力 ±0';
  const signed = (value) => `${value >= 0 ? '+' : ''}${num(value)}`;
  return `今日境界 ${signed(level)} 级 · 战力 ${signed(power)}`;
}


// 实时进度：内核只在整回合结算时发布，两次发布之间快照不动，所以「这一回合
// 走了多远」要按快照里的 `start_unix` + 当前时钟读。**这个数不再由页面算**：
// 内核已经把它做成纯函数（`bongocat_pomodoro::tick_progress_permille`）并从 wasm
// 导出（`web_task_progress` / `web_battle_progress`），`app.js` 的
// `cultivationCtx().live()` 调内核取数，这里只负责画。
//
// 为什么不在这里留一份：网页端、小程序端、App 端原本各有一份抄着的数学，
// 改一处漏两处就是一条会撒谎的条。返回千分比（0..1000）。

// 一条实时进度条。`id` 落在填充块上，好让 paintLiveBars 每帧只改宽度。
function liveBar(id, permille) {
  const pct = Math.max(0, Math.min(100, (permille ?? 0) / 10));
  return `<div class="progress" style="height:6px"><div class="progress-fill" id="${id}" style="width:${pct}%"></div></div>`;
}

// 页面上要画的那两条实时条：任务的这一回合走了多远、整场战斗走了多远。两条
// 同时跑时都画，各自带名字，免得两条细线并排看不出谁是谁。读数是 `ctx.live`
// 给的内核结果，没有运行中的一项是 null。
function liveBars(c, ctx) {
  const { task, battle } = ctx?.live?.() ?? { task: null, battle: null };
  const rows = [];
  if (task != null) {
    rows.push(
      `<div class="live-progress"><span class="live-progress-label">任务</span>${liveBar('live-task-bar', task)}</div>`,
    );
  }
  if (battle != null) {
    rows.push(
      `<div class="live-progress"><span class="live-progress-label">战斗</span>${liveBar('live-battle-bar', battle)}</div>`,
    );
  }
  return rows.join('');
}

// 当前任务那一行：任务 > 闭关 > 战斗 > 空闲。全部读快照，不在这里算数。
function taskLine(c) {
  const task = c.task;
  if (task) {
    const name = TASK_KINDS[task.kind] ?? task.kind ?? '任务';
    const site = task.site ? GATHER_PLACES[task.site]?.[task.tier] ?? SITES[task.site] ?? task.site : '';
    const tier = task.tier ? depthTag(c, task.tier) : '';
    const place = [site, tier].filter(Boolean).join(' · ');
    const grade = task.grade != null ? JOB_GRADE_NAMES[task.grade] ?? '' : '';
    const detail = place || grade;
    const ticks = `${num(task.ticks_done)} 回合 · 每回合 ${task.tick_seconds} 秒`;
    return detail ? `${name} · ${detail} · 已进行 ${ticks}` : `${name} · 已进行 ${ticks}`;
  }
  if (c.seclusion) {
    return `闭关 · 回合 ${num(c.seclusion.ticks_done)}/${num(c.rules?.seclusion_ticks ?? 0)}`;
  }
  const battle = c.battle;
  if (battle) {
    const name = BATTLE_KINDS[battle.kind] ?? battle.kind ?? '战斗';
    const floor = battle.kind === 'Trial' && battle.floor ? ` · 第 ${battle.floor} 层` : '';
    return `${name}${floor} · 回合 ${battle.rounds_done}/${battle.rounds_total}`;
  }
  return '空闲';
}

/* ---------- shared pieces ---------- */

function bar(ratio, cls = '') {
  const width = Math.max(0, Math.min(1, ratio || 0)) * 100;
  return `<div class="bar ${cls}"><i style="width:${width.toFixed(1)}%"></i></div>`;
}

function attributeCells(c) {
  return ATTRIBUTES.map(([key, label]) => {
    const total = c.attributes[key] ?? 0;
    const level = c.level_attributes[key] ?? 0;
    const extra = total - level;
    return `
      <div class="attr">
        <span class="attr-name">${label}</span>
        <span>
          <span class="attr-value">${num(total)}</span>
          ${extra > 0 ? `<span class="attr-gain">+${num(extra)}</span>` : ''}
        </span>
      </div>
    `;
  }).join('');
}

function derivedCells(c) {
  return DERIVED.map(
    ([key, label]) => `
      <div class="attr">
        <span class="attr-name">${label}</span>
        <span class="attr-value">${num(c.derived[key] ?? 0)}</span>
      </div>
    `,
  ).join('');
}

function itemLine(item) {
  const parts = [];
  for (const [key, label] of ATTRIBUTES) {
    const value = item.attributes?.[key] ?? 0;
    if (value) parts.push(`${label} +${num(value)}`);
  }
  for (const [key, label] of DERIVED) {
    const value = item.bonus?.[key] ?? 0;
    if (value) parts.push(`${label} +${num(value)}`);
  }
  return parts.join(' · ') || '无加成';
}

function itemRow(item, c) {
  const [rarityText, rarityCls] = RARITY[item.rarity] ?? RARITY.common;
  const tierText = `${item.tier} 阶`;
  const star = item.star ? ` · ${'★'.repeat(item.star)}` : '';
  const set = item.set ? ` · 套装 ${esc(item.set.name ?? '')}` : '';
  const starCost = item.star_cost
    ? `${num(item.star_cost.stones)} 灵石`
    : '已满星';
  const reforgeCost = item.reforge_cost
    ? `${num(item.reforge_cost.stones)} 灵石、${resourceText(item.reforge_cost.resources)}`
    : '';
  return `
    <div class="row">
      <div class="row-main">
        <div class="row-title">
          <span class="${rarityCls}">${rarityText}</span>
          <span>${esc(item.name ?? '')}</span>
          <span class="chip muted">${tierText}${star}${set}</span>
        </div>
        <div class="row-sub">${itemLine(item)}</div>
      </div>
      <div class="row-actions">
        ${item.equipped
          ? `<button class="btn small" data-act="cult-unequip" data-slot="${item.slot}">卸下</button>`
          : `<button class="btn small jade" data-act="cult-equip" data-id="${item.id}">穿戴</button>`}
        <button class="btn small" data-act="cult-star" data-id="${item.id}" title="升星需要 ${starCost}" ${item.star_cost ? '' : 'disabled'}>升星</button>
        <button class="btn small" data-act="cult-reforge" data-id="${item.id}" title="${item.reforge_cost ? `重铸需要 ${reforgeCost}` : '只有一条属性，无法重铸'}" ${item.reforge_cost ? '' : 'disabled'}>${item.reforge_cost ? '重铸 ' + num(item.reforge_cost.stones) : '无副属性'}</button>
        <button class="btn small danger" data-act="cult-sell-item" data-id="${item.id}">卖 ${num(item.sell_price)}</button>
      </div>
    </div>
  `;
}

/* ---------- status ---------- */

function statusPage(c, snapshot, ctx) {
  const growthSpan = (c.next ?? c.growth) - c.floor;
  const growthRatio = growthSpan > 0 ? (c.growth - c.floor) / growthSpan : 0;
  const breakdown = [
    ['等级', c.power_breakdown.level],
    ['加点', c.power_breakdown.allocated],
    ['装备属性', c.power_breakdown.equipment],
    ['装备加成', c.power_breakdown.equipment_bonus],
    ['灵草', c.power_breakdown.crop],
    ['内功属性', c.power_breakdown.art],
    ['内功加成', c.power_breakdown.art_bonus],
  ];
  const guardian = c.guardian;
  const refusal = refusalText(c.last_refusal);
  return `
    <div class="hero">
      <div class="card">
        <div class="realm-name">${REALMS[c.realm] ?? c.realm}</div>
        <div class="realm-sub">第 ${c.layer + 1} 层 · 第 ${c.rank} 阶 · 等级 ${c.level} / ${c.total_levels}</div>
        <div class="power">
          <span class="power-value">${num(c.power)}</span>
          <span class="power-label">战力</span>
          ${c.power_breakdown.external ? `<span class="chip accent">外功 +${num(c.power_breakdown.external)}</span>` : ''}
        </div>
        ${bar(growthRatio, 'violet')}
        <div class="bar-legend">
          <span>成长 ${num(c.growth)}</span>
          <span>${c.next ? `下一级 ${num(c.next)}` : c.realm_full ? '本境界已满' : '已至顶'}</span>
        </div>
        <div class="wrap" style="margin-top:14px">
          <span class="chip gold">灵石 ${num(c.stones)}</span>
          <span class="chip">潜质 ${(c.aptitude_tenths / 10).toFixed(1)}</span>
          <span class="chip">下次专注 +${num(c.next_session_growth)} 成长</span>
          <span class="chip" data-today-delta="1">${todayDeltaText(c)}</span>
          <span class="chip ${c.at_home ? 'jade' : 'accent'}">${c.at_home ? '在家' : '外出中'}</span>
        </div>
        <div class="row" style="margin-top:14px">
          <div class="row-main">
            <div class="row-title">当前任务</div>
            <div class="row-sub">${taskLine(c)}</div>
            ${liveBars(c, ctx)}
          </div>
        </div>
        ${
          guardian
            ? `
          <div class="row" style="margin-top:14px">
            <div class="row-main">
              <div class="row-title">境界守关者 · ${REALMS[guardian.target] ?? guardian.target}</div>
              <div class="row-sub">战力 ${num(guardian.power)} ｜ 你的战力 ${num(c.power)}</div>
            </div>
            <button class="btn primary" data-act="cult-breakthrough" ${c.breakthrough_ready ? '' : 'disabled'}>
              ${c.power >= guardian.power ? '突破' : `突破（差 ${num(guardian.power - c.power)} 战力）`}
            </button>
          </div>
        `
            : '<div class="card-hint" style="margin-top:14px">本境界还没有走到最后一层。</div>'
        }
        ${
          c.breakthrough_ready_at
            ? `<div class="card-hint" style="margin-top:8px">突破冷却中，还需 ${until(c.breakthrough_ready_at)}。</div>`
            : ''
        }
        ${refusal ? `<div class="card-hint" style="margin-top:8px">上次操作：${refusal}</div>` : ''}
      </div>

      <div class="card">
        <div class="card-head">
          <div class="card-title">四维与战力来源</div>
          <div class="card-hint" style="text-align:right;line-height:1.7">
            ${breakdown.map(([k, v]) => `${k} <b>${num(v)}</b>`).join(' · ')}
          </div>
        </div>
        <div class="attrs">${attributeCells(c)}</div>
        <div class="card-head" style="margin-top:16px">
          <div class="card-title">战斗数值</div>
        </div>
        <div class="attrs">${derivedCells(c)}</div>
      </div>
    </div>

    <div class="grid-2">
      <div class="card">
        <div class="card-head">
          <div class="card-title">分配潜能</div>
          <div class="card-hint">剩余 ${num(c.free_points)} 点</div>
        </div>
        <div class="rows">
          ${ATTRIBUTES.map(
            ([key, label]) => `
            <div class="row">
              <div class="row-main"><div class="row-title">${label}</div><div class="row-sub">已分配 ${num(c.allocated_attributes[key] ?? 0)}</div></div>
              <div class="row-actions">
                <button class="btn small" data-act="cult-alloc" data-attr="${key}" data-points="1" ${c.free_points > 0 ? '' : 'disabled'}>+1</button>
                <button class="btn small" data-act="cult-alloc" data-attr="${key}" data-points="10" ${c.free_points >= 10 ? '' : 'disabled'}>+10</button>
                <button class="btn small jade" data-act="cult-alloc" data-attr="${key}" data-points="${c.free_points}" ${c.free_points > 0 ? '' : 'disabled'}>全部</button>
              </div>
            </div>`,
          ).join('')}
        </div>
      </div>

      <div class="card">
        <div class="card-head">
          <div class="card-title">下次专注的产出</div>
          <div class="card-hint">一次番茄只做一件事</div>
        </div>
        <div class="tabs">
          ${Object.entries(MODES)
            .filter(([key]) => key[0] === key[0].toUpperCase())
            .map(
              ([key, label]) =>
                `<button class="tab ${c.session_mode === key ? 'active' : ''}" data-act="cult-mode" data-mode="${key.toLowerCase()}">${label}</button>`,
            )
            .join('')}
        </div>
        <div class="rows" style="margin-top:12px">
          <div class="row">
            <div class="row-main"><div class="row-title">潜质与丹药</div><div class="row-sub">潜质 ${(c.aptitude_tenths / 10).toFixed(1)}</div></div>
            <div class="row-actions">
              <span class="chip">悟心草 ${c.aptitude_herbs?.[0] ?? 0}</span>
              <span class="chip">慧心草 ${c.aptitude_herbs?.[1] ?? 0}</span>
              <span class="chip">洗髓丹 ${c.aptitude_herbs?.[2] ?? 0}</span>
            </div>
          </div>
          <div class="row">
            <div class="row-main"><div class="row-title">洗髓丹开脉</div><div class="row-sub">${c.marrow_gate_cost ? `需要 ${c.marrow_gate_cost} 颗` : '本段已到顶'}</div></div>
            <div class="row-actions">
              <button class="btn small" data-act="cult-herb" data-herb="insight" ${(c.aptitude_herbs?.[0] ?? 0) > 0 ? '' : 'disabled'}>服悟心草</button>
              <button class="btn small" data-act="cult-herb" data-herb="wisdom" ${(c.aptitude_herbs?.[1] ?? 0) > 0 ? '' : 'disabled'}>服慧心草</button>
              <button class="btn small jade" data-act="cult-marrow" ${c.marrow_gate_cost && (c.aptitude_herbs?.[2] ?? 0) > 0 ? '' : 'disabled'}>服洗髓丹</button>
            </div>
          </div>
          <div class="row">
            <div class="row-main"><div class="row-title">今日机缘</div><div class="row-sub">已发生 ${c.daily_events ?? 0} 次</div></div>
            <div class="row-actions">
              ${
                c.last_daily_event
                  ? `<button class="btn small" data-act="cult-ack" data-day="${c.daily_event_day ?? 0}">收下</button>`
                  : '<span class="chip muted">今日暂无</span>'
              }
            </div>
          </div>
        </div>
      </div>
    </div>
  `;
}

/* ---------- gear ---------- */

function gearPage(c) {
  const equipped = c.equipped ?? [];
  const inventory = c.inventory ?? [];
  return `
    <div class="card">
      <div class="card-head">
        <div class="card-title">穿戴</div>
        <div class="card-hint">装备属性计入战力，套装凑齐另有加成</div>
      </div>
      <div class="rows">
        ${SLOTS.map(([key, label], index) => {
          const item = equipped[index];
          if (!item) {
            return `<div class="row"><div class="row-main"><div class="row-title">${label}</div><div class="row-sub">空置</div></div></div>`;
          }
          return `
            <div class="row">
              <div class="row-main">
                <div class="row-title">${label} · ${esc(item.name ?? '')} ${rarity(item.rarity)} ${item.star ? `<span class="chip gold">${'★'.repeat(item.star)}</span>` : ''}</div>
                <div class="row-sub">${itemLine(item)}</div>
              </div>
              <div class="row-actions">
                <button class="btn small" data-act="cult-unequip" data-slot="${key}">卸下</button>
              </div>
            </div>`;
        }).join('')}
      </div>
      ${
        c.gear_sets?.length
          ? `<div class="chips" style="margin-top:12px">${c.gear_sets
              .map((set) => `<span class="chip gold">${esc(set.set?.name ?? '套装')} ${set.pieces} 件 · ${setBonusText(set.bonus)}${set.effect ? ` · 完整 ${esc(SET_EFFECTS[set.effect] ?? set.effect)}` : ''}</span>`)
              .join('')}</div>`
          : ''
      }
    </div>

    <div class="card">
      <div class="card-head">
        <div class="card-title">背包</div>
        <div class="card-hint">${inventory.length} 件 · 升星赌一半身价</div>
      </div>
      <div class="rows">
        ${inventory.length ? inventory.map((item) => itemRow(item, c)).join('') : '<div class="empty">背包是空的。专注掉落、集市与外出都会带回装备。</div>'}
      </div>
    </div>
  `;
}

function setBonusText(bonus) {
  if (!bonus) return '';
  const parts = [];
  for (const [key, label] of DERIVED) {
    const value = bonus[key] ?? 0;
    if (value) parts.push(`${label}+${num(value)}`);
  }
  return parts.join(' ') || '无';
}

/* ---------- arts ---------- */

function artRow(art, selected) {
  const [gradeText, gradeCls] = GRADE[art.grade] ?? GRADE.Yellow;
  const progress = art.required ? Math.min(1, art.progress / art.required) : 0;
  return `
    <div class="row">
      <div class="row-main">
        <div class="row-title">
          <span class="${gradeCls}">${gradeText}</span>
          <span>${esc(art.name ?? '')}</span>
          <span class="chip muted">${art.kind === 'Internal' ? '内功' : '外功'} · 第 ${art.rank}/${art.max_rank} 重</span>
          ${selected ? '<span class="chip accent">修习中</span>' : ''}
        </div>
        <div class="row-sub">
          成长 ${num(art.progress)}/${num(art.required)} · 突破率 ${(art.success_permille / 10).toFixed(1)}%
          ${art.copies > 1 ? ` · 重复 ${art.copies} 本` : ''}
        </div>
        ${bar(progress, 'gold')}
      </div>
      <div class="row-actions">
        ${
          selected
            ? `<button class="btn small" data-act="cult-select-art" data-art="0">停止修习</button>`
            : `<button class="btn small jade" data-act="cult-select-art" data-art="${art.id}">修习</button>`
        }
        <button class="btn small danger" data-act="cult-sell-art" data-art="${art.id}" ${selected ? 'disabled' : ''}>卖</button>
      </div>
    </div>
  `;
}

function artsPage(c) {
  const arts = c.arts ?? [];
  const mastered = c.mastered_arts ?? [];
  return `
    <div class="card">
      <div class="card-head">
        <div class="card-title">功法</div>
        <div class="card-hint">
          内功属性 +${num(c.art_attributes?.root ?? 0)}/${num(c.art_attributes?.strength ?? 0)}/${num(c.art_attributes?.mana ?? 0)}/${num(c.art_attributes?.agility ?? 0)}
          · 折合战力 ${num(c.art_power)}${c.external_power ? ` · 外功每击 +${num(c.external_power)}` : ''}
        </div>
      </div>
      <div class="rows">
        ${arts.length ? arts.map((art) => artRow(art, art.selected)).join('') : '<div class="empty">还没有功法。外出和集市会给，专注完成也会掉。</div>'}
      </div>
      ${
        c.art_sets?.length
          ? `<div class="chips" style="margin-top:12px">${c.art_sets
              .map((set) => `<span class="chip gold">同门 ${set.books} 本 · ${setBonusText(set.bonus)}</span>`)
              .join('')}</div>`
          : ''
      }
    </div>

    <div class="card">
      <div class="card-head">
        <div class="card-title">已圆满</div>
        <div class="card-hint">${mastered.length} 门</div>
      </div>
      <div class="rows">
        ${
          mastered.length
            ? mastered
                .map(
                  (art) => `
            <div class="row">
              <div class="row-main">
                <div class="row-title">${esc(art.name ?? '')} <span class="chip muted">${art.kind === 'Internal' ? '内功' : '外功'}</span></div>
                <div class="row-sub">${setBonusText(art.bonus)}</div>
              </div>
            </div>`,
                )
                .join('')
            : '<div class="empty">还没有练满的功法。</div>'
        }
      </div>
    </div>
  `;
}

/* ---------- garden ---------- */

function cropLine(crop) {
  const name = crop.name ? esc(crop.name) : crop.herb ? herbName(crop.herb) : '灵草';
  const ripe = crop.ready_at_unix <= Date.now() / 1000;
  const stage = crop.withered
    ? '<span class="chip accent">枯萎</span>'
    : ripe
      ? '<span class="chip jade">可以收获</span>'
      : `<span class="chip">${until(crop.ready_at_unix)}后成熟</span>`;
  return `${name} · ${grade(crop.grade)} ${stage}`;
}

function gardenPage(c) {
  const plots = c.plots ?? [];
  const seeds = c.seeds ?? [];
  const ready = plots.some((plot) => plot.crop && !plot.crop.withered && plot.crop.ready_at_unix <= Date.now() / 1000);
  return `
    <div class="card">
      <div class="card-head">
        <div class="card-title">灵草园</div>
        <div class="card-hint">
          园阶 ${c.garden_rank} · 造化 ${num(c.garden_xp)}/${num(c.garden_xp_needed)}
          · ${plots.length} 块地${ready ? ' · 有可收的' : ''}
        </div>
      </div>
      <div class="form-row" style="margin-bottom:10px">
        <button class="btn small jade" data-act="cult-harvest-all" ${ready ? '' : 'disabled'}>一键收获</button>
        <button class="btn small" data-act="cult-plant-all" ${seeds.length && plots.some((plot) => !plot.crop) ? '' : 'disabled'}>一键种植</button>
      </div>
      ${bar(c.garden_xp_needed ? c.garden_xp / c.garden_xp_needed : 0, 'gold')}
      <div class="rows" style="margin-top:14px">
        ${plots
          .map((plot, index) => {
            const crop = plot.crop;
            if (!crop) {
              return `
                <div class="row">
                  <div class="row-main"><div class="row-title">第 ${index + 1} 块地</div><div class="row-sub">空地，可以种下</div></div>
                  <div class="row-actions"><span class="chip muted">空</span></div>
                </div>`;
            }
            const isReady = !crop.withered && crop.ready_at_unix <= Date.now() / 1000;
            return `
              <div class="row">
                <div class="row-main">
                  <div class="row-title">第 ${index + 1} 块地</div>
                  <div class="row-sub">${cropLine(crop)}</div>
                </div>
                <div class="row-actions">
                  ${
                    isReady
                      ? `<button class="btn small jade" data-act="cult-harvest" data-plot="${index}">收获</button>`
                      : ''
                  }
                  <button class="btn small danger" data-act="cult-dig" data-plot="${index}">挖掉</button>
                </div>
              </div>`;
          })
          .join('')}
      </div>
      <div class="form-row" style="margin-top:14px">
        <button class="btn" data-act="cult-buy-plot" ${c.next_plot_price ? '' : 'disabled'}>
          ${c.next_plot_price ? `买地 ${num(c.next_plot_price)} 灵石` : '地已开满'}
        </button>
        <button class="btn" data-act="cult-upgrade-garden" ${c.garden_upgrade_cost ? '' : 'disabled'}>
          ${c.garden_upgrade_cost ? `升园阶 ${num(c.garden_upgrade_cost)} 灵石` : '园阶已到顶'}
        </button>
      </div>
    </div>

    <div class="card">
      <div class="card-head">
        <div class="card-title">种子</div>
        <div class="card-hint">种下即占一块地，成熟后收成加属性</div>
      </div>
      <div class="rows">
        ${
          seeds.length
            ? seeds
                .map((seed) => {
                  const name = seed.name ? esc(seed.name) : seed.herb ? herbName(seed.herb) : '灵草种子';
                  const attr = seed.herb ? '' : ` · ${ATTRIBUTES.find(([key]) => key === seed.attribute)?.[1] ?? seed.attribute}`;
                  const op = seed.herb ? 'cult-plant-herb' : 'cult-plant';
                  return `
                    <div class="row">
                      <div class="row-main">
                        <div class="row-title">${name} <span class="chip muted">${seed.tier} 阶${attr}</span> ${grade(seed.grade)}</div>
                        <div class="row-sub">持有 ${seed.count} 份 · 单份卖 ${num(seed.sell_price)} 灵石 · 成活率 ${((seed.plant_permille ?? 0) / 10).toFixed(0)}%</div>
                      </div>
                      <div class="row-actions">
                        <button class="btn small jade" data-act="${op}" data-attr="${seed.attribute}" data-tier="${seed.tier}" data-grade="${seed.grade}" data-herb="${seed.herb ?? ''}">种下</button>
                        <button class="btn small danger" data-act="${seed.herb ? 'cult-sell-herb-seed' : 'cult-sell-seed'}" data-attr="${seed.attribute}" data-tier="${seed.tier}" data-grade="${seed.grade}" data-herb="${seed.herb ?? ''}">卖一份</button>
                      </div>
                    </div>`;
                })
                .join('')
            : '<div class="empty">没有种子。外出采集和集市都能找到。</div>'
        }
      </div>
    </div>
  `;
}

/* ---------- market ---------- */

function shopLine(offer) {
  if (!offer) return { title: '未知货物', sub: '', cls: 'muted' };
  if (offer.Equipment) {
    const item = offer.Equipment;
    return {
      title: `${esc(item.name ?? '装备')} ${rarity(item.rarity)}`,
      sub: `${SLOTS.find(([key]) => key === item.slot)?.[1] ?? item.slot} · ${itemLine(item)}`,
      cls: '',
    };
  }
  if (offer.Seed) {
    const seed = offer.Seed;
    const attribute = ATTRIBUTES.find(([key]) => key === seed.attribute)?.[1] ?? seed.attribute;
    return {
      title: seed.name ? esc(seed.name) : `${attribute}灵草种子`,
      sub: `${seed.tier} 阶 · ${attribute} · ${grade(seed.grade)}`,
      cls: '',
    };
  }
  if (offer.Herb) {
    return {
      title: `${herbName(offer.Herb.herb)} 种子`,
      sub: grade(offer.Herb.grade),
      cls: '',
    };
  }
  if (offer.Art) {
    return { title: '功法秘本', sub: `货号 ${offer.Art.art_id} · 买下才知道品阶`, cls: '' };
  }
  if (typeof offer.Stones === 'number') {
    return { title: `${num(offer.Stones)} 灵石`, sub: '一袋灵石', cls: 'gold' };
  }
  return { title: '未知货物', sub: '', cls: '' };
}

function marketPage(c) {
  const offers = c.shop_offers ?? [];
  return `
    <div class="card">
      <div class="card-head">
        <div class="card-title">今日集市</div>
        <div class="card-hint">每日换一批 · 卖价是买价的一定比例</div>
      </div>
      <div class="rows">
        ${offers
          .map((offer) => {
            const line = shopLine(offer.offer);
            // The field is the price as a share of the list price in per-mille,
            // and zero means "at its list price" rather than "free".
            const discount =
              offer.discount_permille > 0
                ? `<span class="chip accent">${(offer.discount_permille / 100).toFixed(1)} 折</span>`
                : '';
            return `
              <div class="row">
                <div class="row-main">
                  <div class="row-title">${line.title} ${discount}</div>
                  <div class="row-sub">${line.sub}</div>
                </div>
                <div class="row-actions">
                  <span class="chip gold">${num(offer.price)}</span>
                  <button class="btn small primary" data-act="cult-buy" data-slot="${offer.slot}" ${offer.bought || offer.price > c.stones ? 'disabled' : ''}>
                    ${offer.bought ? '已售' : offer.price > c.stones ? '灵石不足' : '买下'}
                  </button>
                </div>
              </div>`;
          })
          .join('') || '<div class="empty">货架是空的。</div>'}
      </div>
    </div>
  `;
}

/* ---------- outing ---------- */

function progressBar(percent, opts = {}) {
  const { height = 8 } = opts;
  const pct = Math.max(0, Math.min(100, percent));
  return `<div class="progress" style="height:${height}px"><div class="progress-fill" style="width:${pct}%"></div></div>`;
}

function regionTitle(c) {
  const idx = (c.explore_region_idx ?? c.region_idx ?? 0) | 0;
  return REGIONS[idx]?.[1] ?? `区域 ${idx + 1}`;
}

function regionRow(c, idx) {
  const def = REGIONS[idx] ?? ['', `区域 ${idx + 1}`, ''];
  const cleared = c.explored_levels?.[idx] ?? 0;
  const progress = c.region_progress?.[idx] ?? 0;
  const pct = Math.max(0, Math.min(100, (progress / 1000) * 100));
  const isCurrent = (c.explore_region_idx ?? c.region_idx ?? 0) === idx;
  const currentLevel = isCurrent ? c.explore_level ?? 1 : null;
  const regionLevels = c.rules?.region_levels ?? 0;
  return `
    <div class="region-card${isCurrent ? ' is-current' : ''}" data-region-idx="${idx}">
      <div class="region-card-head">
        <span class="region-card-name">${def[1]}</span>
        <span class="region-card-level">层 ${cleared}/${regionLevels}</span>
      </div>
      ${isCurrent ? `<div class="region-card-sub">当前探索 · 层 ${currentLevel}</div>` : `<div class="region-card-sub">${def[2]}</div>`}
      <div class="region-progress">${progressBar(pct, { height: 6 })}</div>
    </div>
  `;
}

function outingPage(c) {
  const walk = c.last_walk;
  const work = c.work;
  const idx = (c.explore_region_idx ?? c.region_idx ?? 0) | 0;
  const regionLevel = c.explore_level ?? 1;
  const regionProgress = c.region_progress?.[idx] ?? 0;
  const regionLevels = c.rules?.region_levels ?? 0;
  const regionCount = c.rules?.region_count ?? 0;
  return `
    <div class="card">
      <div class="card-head">
        <div class="card-title">外出</div>
        <div class="card-hint">
          ${c.at_home ? '在家，可以出门' : `人在外面，${c.outing_return_seconds ? duration(c.outing_return_seconds) + '后回来' : '归期未定'}`}
        </div>
      </div>
      <div class="rows">
        <div class="row region-row">
          <div class="row-main">
            <div class="row-title">探索 · ${REGIONS[idx]?.[1] ?? `区域 ${idx + 1}`}</div>
            <div class="row-sub">当前层 ${regionLevel}/${regionLevels} · 层满 ${(regionProgress / 10).toFixed(1)}% · 战斗几率更高，材料更好</div>
            ${regionProgress > 0 ? `<div class="region-progress" style="margin-top:6px">${progressBar(regionProgress / 10, { height: 6 })}</div>` : ''}
          </div>
          <div class="row-actions">
            ${(c.rules?.explore_hours ?? [])
              .map(
                (row, index) =>
                  `<button class="btn small${index === 1 ? ' primary' : ''}" data-act="cult-explore" data-hours="${row.hours}" ${c.exploration_ready ? '' : 'disabled'}>${row.hours} 小时 · ×${(row.multiplier_permille / 1000).toFixed(1)}</button>`,
              )
              .join('')}
          </div>
        </div>
        ${
          c.exploration_ready_at
            ? `<div class="row"><div class="row-main"><div class="row-title">探索冷却</div><div class="row-sub">还需 ${until(c.exploration_ready_at)}</div></div></div>`
            : ''
        }
        ${
          c.walk_early || c.work_early
            ? `<div class="row"><div class="row-main"><div class="row-title">提前返程</div><div class="row-sub">路上的时间已经花了，这一趟不再结算</div></div><div class="row-actions"><button class="btn small danger" data-act="cult-return-early">叫人回来</button></div></div>`
            : ''
        }
      </div>
      ${
        c.last_exploration
          ? `<div class="chips" style="margin-top:12px">${findChips(c.last_exploration_found)}</div>`
          : ''
      }
    </div>

    <div class="card">
      <div class="card-head">
        <div class="card-title">区域 · ${regionCount} 区 ${regionLevels} 层</div>
        <div class="card-hint">每个区域 ${regionLevels} 层，层越深战越强、宝越好</div>
      </div>
      <div class="regions-grid">
        ${(c.explored_levels ?? []).map((_, i) => regionRow(c, i)).join('')}
      </div>
    </div>

    <div class="card">
      <div class="card-head">
        <div class="card-title">今日活计</div>
        <div class="card-hint">${work ? `正在干活，${until(work.ends_unix)}后收工` : `刷新一次 ${num(c.job_refresh_cost)} 灵石`}</div>
      </div>
      <div class="rows">
        ${(c.jobs ?? [])
          .map((job) => {
            const first = DERIVED_NAMES[job.kind.first] ?? job.kind.first;
            const second = job.kind.second ? DERIVED_NAMES[job.kind.second] ?? job.kind.second : null;
            const kind = second && second !== first ? `${first} / ${second}` : first;
            const rate = (job.success_rate / 10).toFixed(0);
            const [gradeText, gradeCls] = GRADE[job.grade] ?? GRADE.Yellow;
            return `
              <div class="row">
                <div class="row-main">
                  <div class="row-title">
                    <span class="${gradeCls}">${gradeText}</span>
                    <span>${kind}</span>
                  </div>
                  <div class="row-sub">
                    需要 ${num(job.requirement)}${job.requirement_second ? ` / ${num(job.requirement_second)}` : ''}
                    · 现在 ${num(job.current_first)}${job.current_second ? ` / ${num(job.current_second)}` : ''}
                    · 成功率 ${rate}% · 工钱 ${num(job.hourly_stones)} 灵石/小时
                  </div>
                </div>
                <div class="row-actions">
                  <button class="btn small jade" data-act="cult-job" data-slot="${job.slot}" ${job.working || !c.at_home ? 'disabled' : ''}>
                    ${job.working ? '在做' : '接下'}
                  </button>
                </div>
              </div>`;
          })
          .join('') || '<div class="empty">今天没有活计。</div>'}
      </div>
      <div class="form-row" style="margin-top:12px">
        <button class="btn" data-act="cult-refresh-jobs" ${c.at_home ? '' : 'disabled'}>刷新活计（${num(c.job_refresh_cost)}）</button>
        ${work ? `<button class="btn small danger" data-act="cult-return-early">提前收工</button>` : ''}
      </div>
      ${c.last_work ? `<div class="chips" style="margin-top:12px"><span class="chip jade">上次工钱 ${num(c.last_work.stones ?? 0)} 灵石</span></div>` : ''}
    </div>
  `;
}

function findChips(found) {
  if (!found) return '';
  const chips = [];
  if (found.seed) chips.push(`<span class="chip jade">种子${found.seed.name ? ' ' + esc(found.seed.name) : ''}</span>`);
  if (found.gear) chips.push(`<span class="chip">${esc(found.gear.name ?? '装备')}</span>`);
  if (found.art) chips.push(`<span class="chip gold">功法 ${esc(found.art)}</span>`);
  if (found.stones) chips.push(`<span class="chip gold">${num(found.stones)} 灵石</span>`);
  return chips.join('') || '<span class="chip muted">空手而归</span>';
}

// The region a won bout moved, in tenths of a percent: a loss carries none and
// prints nothing, and a sixth too small to round to a tenth reads as no chip.
function progressChip(progress) {
  if (progress == null) return '';
  return `<span class="chip jade">试炼 +${(progress / 10).toFixed(1)}%</span>`;
}

/* ---------- tower ---------- */

function towerPage(c) {
  const attempts = c.trial_attempts_left ?? 0;
  return `
    <div class="grid-2">
      <div class="card">
        <div class="card-head">
          <div class="card-title">锁妖塔</div>
          <div class="card-hint">已至第 ${c.trial_floor} 层 · 今日剩 ${attempts} 次</div>
        </div>
        <div class="rows">
          <div class="row">
            <div class="row-main">
              <div class="row-title">第 ${c.trial_next_floor} 层守关</div>
              <div class="row-sub">战力 ${num(c.trial_warden_power)} ｜ 你的 ${num(c.power)}</div>
            </div>
            <div class="row-actions">
              <button class="btn small primary" data-act="cult-tower" ${attempts > 0 && c.at_home ? '' : 'disabled'}>挑战</button>
              <button class="btn small" data-act="cult-sweep" ${c.trial_sweepable && attempts > 0 && c.at_home ? '' : 'disabled'}>横扫</button>
            </div>
          </div>
          ${c.trial_blocked ? `<div class="card-hint">${REFUSALS[c.trial_blocked] ?? '暂时不能进塔'}</div>` : ''}
          ${c.last_trial ? `<div class="card-hint">上次闯塔：第 ${c.last_trial.floor} 层${c.last_trial.duel?.won ? '通关' : '未过'}${replayMarkup(c.last_trial.duel)}</div>` : ''}
          ${c.last_trial_sweep ? `<div class="card-hint accent" data-sweep-result="1">${sweepLine(c.last_trial_sweep)}</div>` : ''}
        </div>
      </div>

      <div class="card">
        <div class="card-head">
          <div class="card-title">试炼</div>
          <div class="card-hint">每次专注得一次 · ${c.proving_tokens ?? 0}/${c.proving_token_cap ?? 10}</div>
        </div>
        <div class="rows">
          <div class="row">
            <div class="row-main">
              <div class="row-title">切磋一场</div>
              <div class="row-sub">对手与当前区域进度相匹配，赢了有掉落</div>
            </div>
            <div class="row-actions">
              <button class="btn small primary" data-act="cult-prove" ${(c.proving_tokens ?? 0) > 0 && c.at_home ? '' : 'disabled'}>开始</button>
            </div>
          </div>
          ${c.proving_blocked ? `<div class="card-hint">${REFUSALS[c.proving_blocked] ?? '暂时不能试炼'}</div>` : ''}
          ${c.last_proving ? `<div class="chips">${findChips(c.last_proving.found)}${progressChip(c.last_proving.progress)}</div>` : ''}
        </div>
      </div>
    </div>

    <div class="card">
      <div class="card-head">
        <div class="card-title">最近战报</div>
        <div class="card-hint">${(c.duel_record ?? []).length} 场</div>
      </div>
      <div class="rows">
        ${
          (c.duel_record ?? []).length
            ? c.duel_record
                .slice(-8)
                .reverse()
                .map(
                  (duel) => `
              <div class="row">
                <div class="row-main">
                  <div class="row-title">
                    <span class="chip ${duel.won ? 'jade' : 'accent'}">${duel.won ? '胜' : '负'}</span>
                    <span>${FIGHTS[duel.kind] ?? duel.kind}</span>
                    <span class="chip muted">${REALMS[duel.realm] ?? duel.realm}</span>
                  </div>
                  <div class="row-sub">${when(duel.at_unix)}</div>
                </div>
              </div>`,
                )
                .join('')
            : '<div class="empty">还没有打过。</div>'
        }
      </div>
    </div>
  `;
}

/* ---------- workshop ---------- */

// 实时任务与闭关（`docs/0103` §4）。两件事共用一张卡：任务是「点开始 →
// 实时跑 → 每回合发奖」的那一个槽，闭关不占槽、可以并行，所以它的开始钮
// 不看 `c.task`。全部读数来自快照，页面不自己算进度。
function taskCard(c, ctx) {
  const resources = c.resources ?? {};
  const running = !!c.task;
  // 两个行业各爬各的等级，所以林子和山各站在自己的深度上。页面只读这份读数，
  // 没有可点的阶：深度由内核按等级推导，玩家不用选（`docs/0105`）。
  // 当前深度由 `resources.*.depth` 给；门等级表用来写那句提示，也读 rules。
  const depthLine = (site, trade) =>
    `${GATHER_PLACES[site]?.[trade?.depth ?? 'Common'] ?? site} ${depthTag(c, trade?.depth ?? 'Common')}`;
  const gates = (c.rules?.gather_depths ?? [])
    .filter((row) => (row.unlock_level ?? 0) > 1)
    .map((row) => row.unlock_level)
    .join('、');
  // 打工不走这里：它的开始钮是活计板上的「接下」，接下即开工（`cult-job`）。
  // 吐纳已下线（`docs/0103` §4），它的钮不再出现；老档在途任务仍能读到名字。
  const starts = [
    ['cult-woodcut', '伐木'],
    ['cult-stonecut', '采石'],
  ]
    .map(([act, label]) => `<button class="btn small jade" data-act="${act}" ${running ? 'disabled' : ''}>${label}</button>`)
    .join('');
  const seclusion = c.seclusion;
  const seclusionMinutes = Math.max(1, Math.round((c.rules?.seclusion_seconds ?? 0) / 60));
  return `
    <div class="card">
      <div class="card-head">
        <div class="card-title">实时任务</div>
        <div class="card-hint">${running ? '正在进行，可随时停止' : '当前没有进行中的任务'}</div>
      </div>
      <div class="rows">
        <div class="row">
          <div class="row-main">
            <div class="row-title">当前任务</div>
            <div class="row-sub">${taskLine(c)}</div>
            ${liveBars(c, ctx)}
          </div>
          <div class="row-actions">
            ${running ? '<button class="btn small danger" data-act="cult-stop-task">停止</button>' : ''}
          </div>
        </div>
        <div class="row">
          <div class="row-main">
            <div class="row-title">采集深度</div>
            <div class="row-sub">随采集等级自动加深：升到 ${gates || '—'} 级就自己走到更深处</div>
          </div>
          <div class="row-actions chips">
            <span class="chip muted">伐木 ${depthLine('Forest', resources.woodcut)}</span>
            <span class="chip muted">采石 ${depthLine('Mountain', resources.stonecut)}</span>
          </div>
        </div>
      </div>
      <div class="form-row" style="margin-top:14px">${starts}</div>
    </div>

    <div class="card">
      <div class="card-head">
        <div class="card-title">闭关</div>
        <div class="card-hint">不占任务槽，可与采集或打工并行</div>
      </div>
      <div class="rows">
        <div class="row">
          <div class="row-main">
            <div class="row-title">${seclusion ? `闭关中 · 回合 ${num(seclusion.ticks_done)}/${num(c.rules?.seclusion_ticks ?? 0)}` : '未在闭关'}</div>
            <div class="row-sub">${seclusion ? `坐下 ${seclusionMinutes} 分钟，出关时按回合结算成长` : `点开始坐下 ${seclusionMinutes} 分钟`}</div>
          </div>
          <div class="row-actions">
            ${
              seclusion
                ? '<button class="btn small danger" data-act="cult-stop-seclusion">结束闭关</button>'
                : '<button class="btn small primary" data-act="cult-start-seclusion">开始闭关</button>'
            }
          </div>
        </div>
      </div>
    </div>
  `;
}

function workshopPage(c, ctx) {
  const resources = c.resources ?? {};
  const bag = resources.bag ?? {};
  const cap = resources.cap ?? 0;
  const buildings = [
    ['workshop', '工坊', resources.workshop],
    ['warehouse', '仓库', resources.warehouse],
    ['smithy', '铁匠铺', resources.smithy],
    ['dojo', '道场', resources.dojo],
  ];
  const skills = [
    ['woodcut', '伐木', resources.woodcut],
    ['stonecut', '采石', resources.stonecut],
  ];
  return `
    ${taskCard(c, ctx)}

    <div class="card">
      <div class="card-head">
        <div class="card-title">资源</div>
        <div class="card-hint">上限 ${quantity(cap)} 每样</div>
      </div>
      <div class="attrs">
        ${RESOURCES.map(
          ([key, label]) => `
          <div class="attr">
            <span class="attr-name">${label}</span>
            <span class="attr-value">${quantity(bag[key] ?? 0)}${cap ? ` <span class="muted" style="font-size:11px">/${quantity(cap)}</span>` : ''}</span>
          </div>`,
        ).join('')}
      </div>
      <div class="rows" style="margin-top:14px">
        ${skills
          .map(
            ([key, label, skill]) => `
          <div class="row">
            <div class="row-main">
              <div class="row-title">${label} Lv ${skill?.level ?? 0}</div>
              <div class="row-sub">熟练 ${quantity(skill?.xp ?? 0)}/${quantity(skill?.xp_to_next ?? 0)}</div>
              ${bar(skill?.xp_to_next ? skill.xp / skill.xp_to_next : 0)}
            </div>
          </div>`,
          )
          .join('')}
      </div>
    </div>

    <div class="card">
      <div class="card-head">
        <div class="card-title">建筑</div>
        <div class="card-hint">工坊加产量，仓库存得多，铁匠省钱，道场加成长</div>
      </div>
      <div class="rows">
        ${buildings
          .map(([key, label, building]) => {
            const cost = building?.upgrade_cost;
            const costText = cost
              ? [
                  cost.stones ? `${num(cost.stones)} 灵石` : '',
                  ...RESOURCES.filter(([res]) => (cost.resources?.[res] ?? 0) > 0).map(
                    ([res, resLabel]) => `${resLabel} ${num(cost.resources[res])}`,
                  ),
                ]
                  .filter(Boolean)
                  .join(' · ')
              : '已满级';
            return `
              <div class="row">
                <div class="row-main">
                  <div class="row-title">${label} <span class="chip muted">Lv ${building?.level ?? 0}</span></div>
                  <div class="row-sub">升级需要 ${costText}</div>
                </div>
                <div class="row-actions">
                  <button class="btn small" data-act="cult-build" data-building="${key}" ${building?.capped ? 'disabled' : ''}>
                    ${building?.capped ? '满级' : '升级'}
                  </button>
                  <button class="btn small jade" data-act="cult-build-many" data-building="${key}" data-times="5" ${building?.capped ? 'disabled' : ''}>×5</button>
                </div>
              </div>`;
          })
          .join('')}
      </div>
    </div>
  `;
}

/* ---------- entry ---------- */

export function railCounts(snapshot) {
  const c = snapshot?.cultivation;
  if (!c) return {};
  return {
    focus: snapshot.tasks?.filter((task) => task.status !== 'done').length || 0,
    gear: c.inventory?.length || 0,
    arts: c.arts?.length || 0,
    garden: (c.plots ?? []).filter((plot) => plot.crop).length || 0,
    market: (c.shop_offers ?? []).filter((offer) => !offer.bought).length || 0,
    tower: c.proving_tokens ?? 0,
  };
}

export function renderCultivation(tab, snapshot, ctx) {
  const c = snapshot.cultivation;
  switch (tab) {
    case 'status':
      return statusPage(c, snapshot, ctx);
    case 'record':
      return recordPage(c);
    case 'gear':
      return gearPage(c);
    case 'arts':
      return artsPage(c);
    case 'garden':
      return gardenPage(c);
    case 'market':
      return marketPage(c);
    case 'outing':
      return outingPage(c);
    case 'tower':
      return towerPage(c);
    case 'workshop':
      return workshopPage(c, ctx);
    default:
      return statusPage(c, snapshot, ctx);
  }
}

/* ---------- actions ---------- */

export function handleCultivationAction(data, ctx = {}) {
  const act = data.act;
  if (!act?.startsWith('cult-')) return false;
  const { send } = ctx;
  const payload = (() => {
    switch (act) {
      case 'cult-alloc':
        return { op: 'allocate_attribute', attribute: data.attr, points: Number(data.points) };
      case 'cult-mode':
        return { op: 'select_session_mode', mode: data.mode };
      case 'cult-herb':
        return { op: 'use_aptitude_herb', herb: data.herb };
      case 'cult-marrow':
        return { op: 'use_marrow_pill' };
      case 'cult-ack':
        return { op: 'acknowledge_daily_event', day: Number(data.day) };
      case 'cult-breakthrough':
        return { op: 'breakthrough' };
      case 'cult-equip':
        return { op: 'equip_item', item_id: Number(data.id) };
      case 'cult-unequip':
        return { op: 'unequip_slot', slot: data.slot };
      case 'cult-star':
        return { op: 'star_equipment', item_id: Number(data.id) };
      case 'cult-reforge':
        return { op: 'reforge_equipment', item_id: Number(data.id) };
      case 'cult-sell-item':
        return { op: 'sell_equipment', item_id: Number(data.id) };
      case 'cult-select-art':
        return { op: 'select_art', art_id: Number(data.art) || null };
      case 'cult-sell-art':
        return { op: 'sell_art', art_id: Number(data.art) };
      case 'cult-plant':
        return { op: 'plant_seed', attribute: data.attr, tier: Number(data.tier), grade: data.grade };
      case 'cult-plant-herb':
        return { op: 'plant_herb_seed', herb: data.herb, grade: data.grade };
      case 'cult-harvest':
        return { op: 'harvest_plot', plot: Number(data.plot) };
      case 'cult-harvest-all':
        return { op: 'harvest_all_ready' };
      case 'cult-plant-all':
        return { op: 'plant_all_by_grade' };
      case 'cult-dig':
        // App 是两级确认；网页端用同一句确认文案的原生确认框，语义一致。
        if (
          typeof globalThis.confirm === 'function' &&
          !globalThis.confirm('铲除后无法收获，种子也不会返还。确定铲除？')
        ) {
          return null;
        }
        return { op: 'dig_plot', plot: Number(data.plot) };
      case 'cult-buy-plot':
        return { op: 'buy_plot' };
      case 'cult-upgrade-garden':
        return { op: 'upgrade_garden' };
      case 'cult-sell-seed':
        return { op: 'sell_seed', attribute: data.attr, tier: Number(data.tier), grade: data.grade };
      case 'cult-sell-herb-seed':
        return { op: 'sell_herb_seed', herb: data.herb, grade: data.grade };
      case 'cult-buy':
        return { op: 'buy_from_shop', slot: Number(data.slot) };
      case 'cult-explore':
        return { op: 'explore', hours: Number(data.hours) };
      case 'cult-return-early':
        return { op: 'return_early' };
      // 实时任务（`docs/0103` §4）：吐纳不挑地方，伐木去森林、采石去山。
      // 深度随采集等级自己走（`docs/0105`），所以命令里没有它。打工不走这三个
      // 钮，它在活计板上「接下」即开工。
      // 吐纳已下线（`docs/0103` §4）：老页面的 `cult-meditate` 不再有命令，
      // 这里一并撤掉，避免按钮没了但动作还留着一条死路径。
      case 'cult-woodcut':
        return { op: 'cult_start_task', kind: 'woodcut', site: 'forest' };
      case 'cult-stonecut':
        return { op: 'cult_start_task', kind: 'stonecut', site: 'mountain' };
      case 'cult-stop-task':
        return { op: 'cult_stop_task' };
      case 'cult-start-seclusion':
        return { op: 'cult_start_seclusion' };
      case 'cult-stop-seclusion':
        return { op: 'cult_stop_seclusion' };
      case 'cult-job':
        return { op: 'accept_job', slot: Number(data.slot) };
      case 'cult-refresh-jobs':
        return { op: 'refresh_jobs' };
      case 'cult-tower':
        return { op: 'tower' };
      case 'cult-sweep':
        return { op: 'tower_sweep' };
      case 'cult-prove':
        return { op: 'prove' };
      case 'cult-build':
        return { op: 'upgrade_building', building: data.building };
      case 'cult-build-many':
        return { op: 'upgrade_building_many', building: data.building, times: Number(data.times) };
      default:
        return null;
    }
  })();
  if (!payload) return false;
  send(payload);
  return true;
}
