const store = require('./utils/store');

App({
  globalData: {
    cloudReady: false,
    error: '',
  },

  onLaunch() {
    if (!wx.cloud) {
      this.globalData.error = '当前微信版本太低，请升级后再打开';
      return;
    }
    // 不写死环境 ID：绑定在 AppID 上的默认环境就是它，换环境不用改代码。
    wx.cloud.init({ traceUser: true });
    this.globalData.cloudReady = true;
    // 首屏先拉一次档，四个 tab 共用同一份快照。
    store.boot();
  },
});
