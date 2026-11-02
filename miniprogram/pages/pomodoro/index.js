const store = require('../../utils/store');
const V = require('../../utils/views');
const L = require('../../utils/labels');

Page({
  data: {
    view: null,
    loading: true,
    error: '',
    newName: '',
    estimate: 4,
    estimateOptions: [1, 2, 3, 4, 6, 8, 12],
    estimateLabels: ['1 个', '2 个', '3 个', '4 个', '6 个', '8 个', '12 个'],
    busy: false,
  },

  onLoad() {
    this.cursor = null;
  },

  onShow() {
    store.startTick({ clock: () => this.paintClock(), full: () => this.render() });
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

  /* ---------- 渲染 ---------- */

  render() {
    const snapshot = store.state.snapshot;
    if (!snapshot) {
      this.setData({ loading: true, error: store.state.error || '' });
      return;
    }
    this.setData({
      loading: false,
      error: '',
      view: V.pomodoro(snapshot, store.state.calendar, this.cursor, store.remainingSeconds()),
    });
  },

  // 每秒只改这两个字段，避免整页 setData。
  paintClock() {
    const view = this.data.view;
    if (!view || !view.running) return;
    const left = store.remainingSeconds();
    this.setData({
      'view.clock': L.clock(left),
      'view.ratio': Math.round(Math.max(0, Math.min(1, left / (view.totalSeconds || 1))) * 100),
    });
  },

  /* ---------- 请求 ---------- */

  async send(payload, successText) {
    if (this.data.busy) return;
    this.setData({ busy: true });
    try {
      const result = await store.send(payload.op, payload);
      this.render();
      if (result.completed) {
        wx.showToast({ title: '一次专注完成，修为到账', icon: 'none' });
      } else if (result.refused) {
        const reason = L.refusalText(result.snapshot.cultivation.last_refusal);
        wx.showToast({ title: reason || '这个操作没有生效', icon: 'none' });
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

  /* ---------- 事件 ---------- */

  onNameInput(event) {
    this.setData({ newName: event.detail.value });
  },

  onEstimateChange(event) {
    this.setData({ estimate: this.data.estimateOptions[Number(event.detail.value)] || 1 });
  },

  onAction(event) {
    const data = event.currentTarget.dataset;
    const id = Number(data.id || 0);
    switch (data.act) {
      case 'retry':
        this.setData({ loading: true });
        store
          .boot()
          .then(() => this.render())
          .catch((error) => this.setData({ loading: false, error: error.message }));
        return;
      case 'start':
        this.send({ op: 'start_task', task_id: id });
        return;
      case 'pause':
        this.send({ op: 'pause' });
        return;
      case 'resume':
        this.send({ op: 'resume' });
        return;
      case 'abandon':
        wx.showModal({
          title: '放弃这次专注？',
          content: '已经过去的时间不会折算成任何修为。',
          confirmText: '放弃',
          confirmColor: '#b4553f',
          success: (res) => {
            if (res.confirm) this.send({ op: 'abandon' }, '这次专注已放弃');
          },
        });
        return;
      case 'toggle':
        this.send({ op: 'set_task_done', task_id: id, done: !data.done });
        return;
      case 'more':
        this.openTaskMenu(id);
        return;
      case 'minutes':
        this.send({ op: 'set_focus_minutes', minutes: Number(data.value) }, `每次 ${data.value} 分钟`);
        return;
      case 'create':
        this.createTask();
        return;
      case 'cal-prev':
      case 'cal-next':
        this.shiftMonth(data.act === 'cal-prev' ? -1 : 1);
        return;
      default:
        return;
    }
  },

  createTask() {
    const name = (this.data.newName || '').trim();
    if (!name) {
      wx.showToast({ title: '给任务起个名字', icon: 'none' });
      return;
    }
    const estimate = Math.min(99, Math.max(1, Number(this.data.estimate) || 1));
    this.setData({ newName: '' });
    this.send({ op: 'create_task', name: name.slice(0, 80), estimated: estimate }, '任务已添加');
  },

  openTaskMenu(id) {
    const task = (this.data.view.tasks || []).find((item) => item.id === id);
    if (!task) return;
    wx.showActionSheet({
      itemList: ['改名', '改预计番茄', task.done ? '标记未完成' : '标记完成', '删除任务'],
      success: (res) => {
        if (res.tapIndex === 0) this.renameTask(task);
        if (res.tapIndex === 1) this.reestimateTask(task);
        if (res.tapIndex === 2) this.send({ op: 'set_task_done', task_id: id, done: !task.done });
        if (res.tapIndex === 3) {
          wx.showModal({
            title: '删除任务',
            content: `「${task.name}」的记录会一起删除。`,
            confirmText: '删除',
            confirmColor: '#b4553f',
            success: (confirm) => {
              if (confirm.confirm) this.send({ op: 'delete_task', task_id: id }, '任务已删除');
            },
          });
        }
      },
    });
  },

  renameTask(task) {
    wx.showModal({
      title: '改个名字',
      editable: true,
      placeholderText: '任务名',
      content: task.name,
      success: (res) => {
        if (!res.confirm) return;
        const name = (res.content || '').trim();
        if (!name) {
          wx.showToast({ title: '名字不能为空', icon: 'none' });
          return;
        }
        this.send({ op: 'update_task', task_id: task.id, name: name.slice(0, 80), estimated: Number(task.countText.split('/')[1]) || 1 });
      },
    });
  },

  reestimateTask(task) {
    const current = Number(task.countText.split('/')[1]) || 1;
    wx.showActionSheet({
      itemList: ['1 个', '2 个', '3 个', '4 个', '6 个', '8 个', '12 个'],
      success: (res) => {
        const options = [1, 2, 3, 4, 6, 8, 12];
        const estimated = options[res.tapIndex] || current;
        this.send({ op: 'update_task', task_id: task.id, name: task.name, estimated });
      },
    });
  },

  shiftMonth(delta) {
    const cal = this.data.view && this.data.view.cal;
    const base = cal ? { year: cal.year, month: cal.month } : this.cursor || { year: new Date().getFullYear(), month: new Date().getMonth() };
    const month = base.month + delta;
    this.cursor = { year: base.year + Math.floor(month / 12), month: ((month % 12) + 12) % 12 };
    this.render();
  },
});
