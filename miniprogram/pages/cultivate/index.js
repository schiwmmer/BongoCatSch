const store = require('../../utils/store');
const V = require('../../utils/views');
const L = require('../../utils/labels');

const SECTIONS = [
  { key: 'outing', label: '外出' },
  { key: 'tower', label: '塔与试炼' },
  { key: 'workshop', label: '洞府' },
  { key: 'garden', label: '灵草园' },
  { key: 'market', label: '集市' },
];

Page({
  data: {
    sections: SECTIONS,
    section: 'outing',
    view: null,
    header: null,
    loading: true,
    error: '',
    busy: false,
    taskTier: 'common',
  },

  onLoad(query) {
    const wanted = query && query.section;
    if (wanted && SECTIONS.some((item) => item.key === wanted)) {
      this.setData({ section: wanted });
    }
  },

  onShow() {
    store.startTick({ clock: () => this.paintLiveBars(), full: () => this.render() });
    if (!store.state.snapshot) {
      this.setData({ loading: true, error: '' });
      store.boot().then(() => this.render());
    } else {
      this.render();
    }
  },

  onHide() {
    store.stopTick();
  },

  onUnload() {
    store.stopTick();
  },

  onPullDownRefresh() {
    store
      .refresh()
      .then(() => this.render())
      .catch((error) => wx.showToast({ title: error.message, icon: 'none' }))
      .finally(() => wx.stopPullDownRefresh());
  },

  render() {
    const snapshot = store.state.snapshot;
    if (!snapshot) {
      this.setData({ loading: true, error: store.state.error || '' });
      return;
    }
    const c = snapshot.cultivation;
    let view;
    switch (this.data.section) {
      case 'tower':
        view = V.tower(c);
        break;
      case 'workshop':
        view = V.workshop(c, this.data.taskTier);
        break;
      case 'garden':
        view = V.garden(c);
        break;
      case 'market':
        view = V.market(c);
        break;
      default:
        view = V.outing(c);
        break;
    }
    this.setData({ loading: false, error: '', view, header: V.header(snapshot) });
  },

  /// 实时进度条：每秒只回画两条宽度，不重渲染整页（整页是 30 秒一次的 `full`）。
  /// 条按快照的 `start_unix` 和当前时钟自己算，与 App/web 同一口径。
  paintLiveBars() {
    if (this.data.section !== 'workshop' || !this.data.view) return;
    const c = store.state.snapshot && store.state.snapshot.cultivation;
    if (!c) return;
    const live = V.liveProgress(c, Date.now());
    if (live.task == null && live.battle == null) return;
    this.setData({
      'view.task.live': true,
      'view.task.taskBar': live.task != null,
      'view.task.battleBar': live.battle != null,
      'view.task.taskPercent': V.bar((live.task || 0) / 1000),
      'view.task.battlePercent': V.bar((live.battle || 0) / 1000),
    });
  },

  async send(payload, successText) {
    if (this.data.busy) return;
    this.setData({ busy: true });
    try {
      const result = await store.send(payload.op, payload);
      this.render();
      if (result.refused) {
        wx.showToast({ title: L.refusalText(result.snapshot.cultivation.last_refusal) || '这个操作没有生效', icon: 'none' });
      } else if (result.completed) {
        wx.showToast({ title: '一次专注完成，修为到账', icon: 'none' });
      } else if (successText) {
        wx.showToast({ title: successText, icon: 'none' });
      }
    } catch (error) {
      wx.showToast({ title: error.message || '请求失败', icon: 'none' });
      this.render();
    } finally {
      this.setData({ busy: false });
    }
  },

  onSection(event) {
    const section = event.currentTarget.dataset.section;
    if (!section || section === this.data.section) return;
    this.setData({ section });
    this.render();
  },

  onAction(event) {
    const data = event.currentTarget.dataset;
    switch (data.act) {
      case 'retry':
        this.setData({ loading: true });
        store
          .boot()
          .then(() => this.render())
          .catch((error) => this.setData({ loading: false, error: error.message }));
        return;
      case 'task-tier':
        this.setData({ taskTier: data.tier });
        this.render();
        return;
      case 'cult-meditate':
        this.send({ op: 'cult_start_task', kind: 'meditate' });
        return;
      case 'cult-woodcut':
        this.send({
          op: 'cult_start_task',
          kind: 'woodcut',
          site: 'forest',
          tier: this.data.taskTier,
        });
        return;
      case 'cult-stonecut':
        this.send({
          op: 'cult_start_task',
          kind: 'stonecut',
          site: 'mountain',
          tier: this.data.taskTier,
        });
        return;
      case 'cult-stop-task':
        this.send({ op: 'cult_stop_task' }, '已停止');
        return;
      case 'cult-start-seclusion':
        this.send({ op: 'cult_start_seclusion' }, '已开始闭关');
        return;
      case 'cult-stop-seclusion':
        this.send({ op: 'cult_stop_seclusion' }, '已结束闭关');
        return;
      case 'explore':
        this.send({ op: 'explore', hours: Number(data.hours) });
        return;
      case 'return-early': {
        // S4 之后打工是实时任务，`return_early` 只叫得动探索/旧档 hour-job，
        // 叫不动 Work 任务。按当前任务的 kind 分发：是打工任务就发停止任务。
        const cultivation = (store.state.snapshot && store.state.snapshot.cultivation) || {};
        const task = cultivation.task;
        if (task && task.kind === 'Work') {
          this.send({ op: 'cult_stop_task' }, '已收工');
          return;
        }
        wx.showModal({
          title: '提前返回？',
          content: '路上花掉的时间不会折算成收益。',
          confirmText: '回来',
          success: (res) => {
            if (res.confirm) this.send({ op: 'return_early' });
          },
        });
        return;
      }
      case 'job':
        this.send({ op: 'accept_job', slot: Number(data.slot) });
        return;
      case 'refresh-jobs':
        this.send({ op: 'refresh_jobs' }, '活计已刷新');
        return;
      case 'tower':
        this.send({ op: 'tower' });
        return;
      case 'sweep':
        this.send({ op: 'tower_sweep' });
        return;
      case 'prove':
        this.send({ op: 'prove' });
        return;
      case 'build':
        this.send({ op: 'upgrade_building', building: data.building });
        return;
      case 'build-many':
        this.send({ op: 'upgrade_building_many', building: data.building, times: 5 });
        return;
      case 'harvest':
        this.send({ op: 'harvest_plot', plot: Number(data.plot) }, '已收获');
        return;
      case 'dig':
        wx.showModal({
          title: '挖掉这株？',
          content: '地里的作物会一起清除，没有收成。',
          confirmText: '挖掉',
          confirmColor: '#b4553f',
          success: (res) => {
            if (res.confirm) this.send({ op: 'dig_plot', plot: Number(data.plot) });
          },
        });
        return;
      case 'buy-plot':
        this.send({ op: 'buy_plot' }, '新地已开');
        return;
      case 'upgrade-garden':
        this.send({ op: 'upgrade_garden' }, '园阶已提升');
        return;
      case 'plant':
        this.send({
          op: 'plant_seed',
          attribute: data.attribute,
          tier: Number(data.tier),
          grade: data.grade,
        }, '已种下');
        return;
      case 'plant-herb':
        this.send({ op: 'plant_herb_seed', herb: data.herb, grade: data.grade }, '已种下');
        return;
      case 'sell-seed':
        this.send({
          op: 'sell_seed',
          attribute: data.attribute,
          tier: Number(data.tier),
          grade: data.grade,
        }, '卖出一份');
        return;
      case 'sell-herb-seed':
        this.send({ op: 'sell_herb_seed', herb: data.herb, grade: data.grade }, '卖出一份');
        return;
      case 'buy':
        this.send({ op: 'buy_from_shop', slot: Number(data.slot) }, '买下了');
        return;
      default:
        return;
    }
  },
});
