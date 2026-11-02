const store = require('../../utils/store');
const V = require('../../utils/views');
const L = require('../../utils/labels');

const SECTIONS = [
  { key: 'status', label: '状态' },
  { key: 'arts', label: '功法' },
  { key: 'gear', label: '装备' },
];

Page({
  data: {
    sections: SECTIONS,
    section: 'status',
    view: null,
    header: null,
    loading: true,
    error: '',
    busy: false,
  },

  onLoad(query) {
    const wanted = query && query.section;
    if (wanted && SECTIONS.some((item) => item.key === wanted)) {
      this.setData({ section: wanted });
    }
  },

  onShow() {
    store.startTick({ full: () => this.render() });
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
    if (this.data.section === 'arts') view = V.artsSection(c);
    else if (this.data.section === 'gear') view = V.gearSection(c);
    else view = V.statusSection(c);
    this.setData({ loading: false, error: '', view, header: V.header(snapshot) });
  },

  async send(payload, successText) {
    if (this.data.busy) return;
    this.setData({ busy: true });
    try {
      const result = await store.send(payload.op, payload);
      this.render();
      if (result.refused) {
        wx.showToast({
          title: L.refusalText(result.snapshot.cultivation.last_refusal) || '这个操作没有生效',
          icon: 'none',
        });
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
      case 'alloc':
        this.send({ op: 'allocate_attribute', attribute: data.attr, points: Number(data.points) });
        return;
      case 'mode':
        this.send({ op: 'select_session_mode', mode: data.mode });
        return;
      case 'herb':
        this.send({ op: 'use_aptitude_herb', herb: data.herb });
        return;
      case 'marrow':
        this.send({ op: 'use_marrow_pill' });
        return;
      case 'ack':
        this.send({ op: 'acknowledge_daily_event', day: Number(data.day) });
        return;
      case 'breakthrough':
        this.send({ op: 'breakthrough' });
        return;
      case 'equip':
        this.send({ op: 'equip_item', item_id: Number(data.id) }, '已穿戴');
        return;
      case 'unequip':
        this.send({ op: 'unequip_slot', slot: data.slot }, '已卸下');
        return;
      case 'star':
        this.send({ op: 'star_equipment', item_id: Number(data.id) });
        return;
      case 'sell-item':
        wx.showModal({
          title: '卖掉这件装备？',
          content: '卖出去就留在集市了，拿不回来。',
          confirmText: '卖掉',
          confirmColor: '#b4553f',
          success: (res) => {
            if (res.confirm) this.send({ op: 'sell_equipment', item_id: Number(data.id) }, '已卖出');
          },
        });
        return;
      case 'select-art':
        this.send({ op: 'select_art', art_id: Number(data.art) || null });
        return;
      case 'sell-art':
        wx.showModal({
          title: '卖掉这门功法？',
          content: '卖掉后这本书就没了。',
          confirmText: '卖掉',
          confirmColor: '#b4553f',
          success: (res) => {
            if (res.confirm) this.send({ op: 'sell_art', art_id: Number(data.art) }, '已卖出');
          },
        });
        return;
      default:
        return;
    }
  },
});
