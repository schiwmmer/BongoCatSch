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
  Out: '人不在家',
  Working: '正在打工',
  Spent: '次数已用完',
  Drained: '今日次数已用完',
  Top: '已经到顶',
};

export const CULTIVATION_TABS = [
  { id: 'status', label: '状态' },
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
  return REFUSALS[key] ?? '条件不足';
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
  const tierText = `${'一'.repeat(1)}${item.tier} 阶`;
  const star = item.star ? ` · ${'★'.repeat(item.star)}` : '';
  const set = item.set ? ` · 套装 ${esc(item.set.name ?? '')}` : '';
  const starCost = item.star_cost
    ? `${num(item.star_cost.stones)} 灵石`
    : '已满星';
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
          <span class="chip ${c.at_home ? 'jade' : 'accent'}">${c.at_home ? '在家' : '外出中'}</span>
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
              .map((set) => `<span class="chip gold">${esc(set.set?.name ?? '套装')} ${set.pieces} 件 · ${setBonusText(set.bonus)}</span>`)
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
                        <div class="row-sub">持有 ${seed.count} 份 · 单份卖 ${num(seed.sell_price)} 灵石</div>
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

function outingPage(c) {
  const expedition = c.resources?.expedition;
  const walk = c.last_walk;
  const work = c.work;
  return `
    <div class="card">
      <div class="card-head">
        <div class="card-title">外出</div>
        <div class="card-hint">
          ${c.at_home ? '在家，可以出门' : `人在外面，${c.outing_return_seconds ? duration(c.outing_return_seconds) + '后回来' : '归期未定'}`}
        </div>
      </div>
      <div class="rows">
        <div class="row">
          <div class="row-main">
            <div class="row-title">探索</div>
            <div class="row-sub">带回装备、功法、草药和灵石；时长越久收获越多</div>
          </div>
          <div class="row-actions">
            ${[4, 8, 12]
              .map(
                (hours) =>
                  `<button class="btn small${hours === 8 ? ' primary' : ''}" data-act="cult-explore" data-hours="${hours}" ${c.exploration_ready ? '' : 'disabled'}>${hours} 小时</button>`,
              )
              .join('')}
          </div>
        </div>
        ${
          c.exploration_ready_at
            ? `<div class="row"><div class="row-main"><div class="row-title">探索冷却</div><div class="row-sub">还需 ${until(c.exploration_ready_at)}</div></div></div>`
            : ''
        }
        <div class="row">
          <div class="row-main">
            <div class="row-title">采集远征</div>
            <div class="row-sub">
              ${expedition ? `${SITES[expedition.site]} · ${expedition.hours} 小时 · ${until(expedition.ends_unix)}后归` : '去森林、山麓或矿洞搬木石铁煤'}
            </div>
          </div>
          <div class="row-actions">
            ${
              expedition
                ? `<button class="btn small danger" data-act="cult-return-early">提前收回</button>`
                : ['forest', 'mountain', 'mine']
                    .map(
                      (site) =>
                        `<button class="btn small" data-act="cult-expedition" data-site="${site}" data-hours="8">${SITES[site]} 8 小时</button>`,
                    )
                    .join('')
            }
          </div>
        </div>
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
                    <span class="chip muted">${job.hours} 小时</span>
                  </div>
                  <div class="row-sub">
                    需要 ${num(job.requirement)}${job.requirement_second ? ` / ${num(job.requirement_second)}` : ''}
                    · 现在 ${num(job.current_first)}${job.current_second ? ` / ${num(job.current_second)}` : ''}
                    · 成功率 ${rate}% · 工钱 ${num(job.stones)} 灵石
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
              <div class="row-sub">对手与你同级，赢了有掉落</div>
            </div>
            <div class="row-actions">
              <button class="btn small primary" data-act="cult-prove" ${(c.proving_tokens ?? 0) > 0 && c.at_home ? '' : 'disabled'}>开始</button>
            </div>
          </div>
          ${c.proving_blocked ? `<div class="card-hint">${REFUSALS[c.proving_blocked] ?? '暂时不能试炼'}</div>` : ''}
          ${c.last_proving ? `<div class="chips">${findChips(c.last_proving.found)}</div>` : ''}
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

function workshopPage(c) {
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
    <div class="card">
      <div class="card-head">
        <div class="card-title">资源</div>
        <div class="card-hint">上限 ${num(cap)} 每样</div>
      </div>
      <div class="attrs">
        ${RESOURCES.map(
          ([key, label]) => `
          <div class="attr">
            <span class="attr-name">${label}</span>
            <span class="attr-value">${num(bag[key] ?? 0)}${cap ? ` <span class="muted" style="font-size:11px">/${num(cap)}</span>` : ''}</span>
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
              <div class="row-sub">熟练 ${num(skill?.xp ?? 0)}/${num(skill?.xp_to_next ?? 0)}</div>
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
      return workshopPage(c);
    default:
      return statusPage(c, snapshot, ctx);
  }
}

/* ---------- actions ---------- */

export function handleCultivationAction(data, send, toast) {
  const act = data.act;
  if (!act?.startsWith('cult-')) return false;
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
      case 'cult-dig':
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
      case 'cult-expedition':
        return { op: 'start_expedition', site: data.site, hours: Number(data.hours) };
      case 'cult-return-early':
        return { op: 'return_early' };
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
