// 文案与格式化。数值全部来自服务端快照，这里只做「数字 → 人话」的翻译。

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
  common: '凡品',
  fine: '良品',
  rare: '珍品',
  epic: '奇珍',
  legendary: '神品',
};

const GRADE = {
  Yellow: '黄阶',
  Profound: '玄阶',
  Earth: '地阶',
  Heaven: '天阶',
  Divine: '神阶',
  yellow: '黄阶',
  profound: '玄阶',
  earth: '地阶',
  heaven: '天阶',
  divine: '神阶',
};

const HERBS = {
  Insight: '悟心草',
  Wisdom: '慧心草',
  Marrow: '洗髓丹',
  insight: '悟心草',
  wisdom: '慧心草',
  marrow: '洗髓丹',
};

const SITES = {
  Forest: '森林',
  Mountain: '山麓',
  Mine: '矿洞',
  forest: '森林',
  mountain: '山麓',
  mine: '矿洞',
};

const SITE_KEYS = ['forest', 'mountain', 'mine'];

const BUILDINGS = [
  ['workshop', '工坊', '产量'],
  ['warehouse', '仓库', '存量'],
  ['smithy', '铁匠铺', '省料'],
  ['dojo', '道场', '成长'],
];

const MODES = [
  ['Study', '修习'],
  ['Woodcut', '伐木'],
  ['Mine', '采石'],
];

const REGIONS = [
  ['azure_peaks', '青云山脉'],
  ['frozen_wastes', '玄冰荒漠'],
  ['flame_caverns', '赤炎洞窟'],
  ['shadow_realm', '幽冥鬼域'],
  ['celestial_court', '天外仙境'],
];

const RESOURCES = [
  ['wood', '木'],
  ['stone', '石'],
  ['iron', '铁'],
  ['coal', '煤'],
];

const FIGHTS = {
  Guardian: '境界守关者',
  Wanderer: '游历散修',
  Trial: '塔中对手',
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
  Out: '人不在家',
  Spent: '次数已用完',
  Drained: '今日次数已用完',
  Top: '已经到顶',
};

/// 千分位。小程序部分机型没有 Intl，自己拼更稳。
function num(value) {
  const number = Number(value);
  if (!isFinite(number)) return '0';
  const negative = number < 0;
  const text = String(Math.abs(Math.round(number)));
  let out = '';
  for (let index = 0; index < text.length; index += 1) {
    if (index > 0 && (text.length - index) % 3 === 0) out += ',';
    out += text[index];
  }
  return negative ? `-${out}` : out;
}

function clock(seconds) {
  const total = Math.max(0, Math.round(Number(seconds) || 0));
  const mm = String(Math.floor(total / 60)).padStart(2, '0');
  const ss = String(total % 60).padStart(2, '0');
  return `${mm}:${ss}`;
}

function duration(seconds) {
  const total = Math.max(0, Math.round(Number(seconds) || 0));
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
  if (!unix) return '';
  const now = Date.now() / 1000;
  const left = Number(unix) - now;
  if (left <= 0) return '马上';
  return duration(left);
}

function when(unix) {
  if (!unix) return '';
  const date = new Date(Number(unix) * 1000);
  const pad = (value) => String(value).padStart(2, '0');
  return `${date.getMonth() + 1} 月 ${date.getDate()} 日 ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function gradeText(value) {
  return GRADE[value] || '黄阶';
}

function rarityText(value) {
  return RARITY[value] || '凡品';
}

function herbText(value) {
  return HERBS[value] || String(value || '');
}

function realmText(value) {
  return REALMS[value] || String(value || '');
}

function attributeText(value) {
  const found = ATTRIBUTES.find(([key]) => key === value);
  return found ? found[1] : String(value || '');
}

function refusalText(refusal) {
  if (!refusal) return '';
  if (typeof refusal === 'string') return REFUSALS[refusal] || '条件不足';
  const key = Object.keys(refusal)[0];
  if (key === 'Trade') {
    const detail = refusal.Trade;
    return detail === 'NotOwned' ? '手里没有这件东西' : '交易没有成立';
  }
  return REFUSALS[key] || '条件不足';
}

/// 装备/功法的一行加成：「根骨 +3 · 攻击 +12」
function bonusLine(attributes, bonus) {
  const parts = [];
  for (const [key, label] of ATTRIBUTES) {
    const value = attributes && attributes[key];
    if (value) parts.push(`${label} +${num(value)}`);
  }
  for (const [key, label] of DERIVED) {
    const value = bonus && bonus[key];
    if (value) parts.push(`${label} +${num(value)}`);
  }
  return parts.join(' · ') || '无加成';
}

module.exports = {
  ATTRIBUTES,
  BUILDINGS,
  DERIVED,
  DERIVED_NAMES,
  FIGHTS,
  GRADE,
  HERBS,
  MODES,
  RARITY,
  REFUSALS,
  REGIONS,
  REALMS,
  RESOURCES,
  SITE_KEYS,
  SITES,
  SLOTS,
  attributeText,
  bonusLine,
  clock,
  duration,
  gradeText,
  herbText,
  num,
  rarityText,
  realmText,
  refusalText,
  until,
  when,
};
