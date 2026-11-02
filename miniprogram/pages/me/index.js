const store = require('../../utils/store');
const V = require('../../utils/views');

Page({
  data: {
    view: null,
    loading: true,
    error: '',
    syncText: '已同步',
    syncBusy: false,
    nicknameDraft: '',
    joined: '',
    accountTail: '',
    busy: false,
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
    const account = store.state.account || {};
    const joinedAt = account.joinedAt ? new Date(account.joinedAt) : null;
    const days = joinedAt ? Math.max(1, Math.ceil((Date.now() - joinedAt.getTime()) / 86400000)) : 0;
    const busy = store.state.status === 'busy';
    this.setData({
      loading: false,
      error: '',
      view: V.me(snapshot, store.state.profile),
      nicknameDraft: (store.state.profile && store.state.profile.nickname) || '',
      accountTail: account.tail ? `尾号 ${account.tail}` : '',
      joined: joinedAt
        ? `${joinedAt.getFullYear()} 年 ${joinedAt.getMonth() + 1} 月 ${joinedAt.getDate()} 日入门 · 第 ${days} 天`
        : '',
      syncText: busy ? '同步中…' : store.state.error ? '未同步' : '已同步',
      syncBusy: busy,
    });
  },

  onNicknameInput(event) {
    this.setData({ nicknameDraft: event.detail.value });
  },

  onNicknameConfirm() {
    const nickname = (this.data.nicknameDraft || '').trim();
    if (!nickname) {
      wx.showToast({ title: '昵称不能为空', icon: 'none' });
      return;
    }
    this.saveProfile({ nickname });
  },

  onChooseAvatar(event) {
    const tempPath = event.detail.avatarUrl;
    if (!tempPath) return;
    wx.showLoading({ title: '上传头像' });
    wx.cloud
      .uploadFile({
        cloudPath: `avatars/${Date.now()}-${Math.floor(Math.random() * 10000)}.png`,
        filePath: tempPath,
      })
      .then((res) => {
        wx.hideLoading();
        return this.saveProfile({ avatar: res.fileID });
      })
      .catch((error) => {
        wx.hideLoading();
        wx.showToast({ title: (error && error.message) || '头像上传失败', icon: 'none' });
      });
  },

  async saveProfile(patch) {
    if (this.data.busy) return;
    this.setData({ busy: true });
    try {
      await store.profile(patch);
      this.render();
      wx.showToast({ title: '已保存', icon: 'none' });
    } catch (error) {
      wx.showToast({ title: error.message || '保存失败', icon: 'none' });
    } finally {
      this.setData({ busy: false });
    }
  },

  onAction(event) {
    switch (event.currentTarget.dataset.act) {
      case 'refresh':
        store
          .refresh()
          .then(() => this.render())
          .catch((error) => wx.showToast({ title: error.message, icon: 'none' }));
        return;
      case 'retry':
        this.setData({ loading: true });
        store
          .boot()
          .then(() => this.render())
          .catch((error) => this.setData({ loading: false, error: error.message }));
        return;
      case 'rule':
        wx.showModal({
          title: '数据是怎么算的',
          content:
            '番茄钟在你手机上走秒，但开始、暂停、完成都由服务端按服务器时间结算：一次专注从开始到结束不满 25 分钟不会产出。所有修为、灵石、掉落也在服务端算，手机只负责显示。',
          showCancel: false,
          confirmText: '知道了',
        });
        return;
      default:
        return;
    }
  },
});
