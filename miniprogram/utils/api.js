// 与云函数的唯一通道。小程序端没有任何游戏逻辑，只有「发操作、收快照」。

const FUNCTION_NAME = 'game';

function describe(error) {
  const raw = (error && (error.errMsg || error.message)) || String(error || '未知错误');
  if (/FunctionName|FUNCTION_NOT_FOUND|not found/i.test(raw)) {
    return '云函数 game 还没部署，先在开发者工具里「上传并部署」';
  }
  if (/env|environment/i.test(raw) && /not|invalid/i.test(raw)) {
    return '云开发环境未开通或未绑定，请先在开发者工具里开通云开发';
  }
  if (/timeout|time out|网络|network/i.test(raw)) {
    return '网络不太通，稍后会自动重试';
  }
  return raw;
}

/// 发一条请求。成功返回云函数的 result，失败抛出带 `code` 的错误。
function call(op, payload) {
  return new Promise((resolve, reject) => {
    if (!wx.cloud || !wx.cloud.callFunction) {
      reject(Object.assign(new Error('当前基础库不支持云开发'), { code: 'NO_CLOUD' }));
      return;
    }
    wx.cloud.callFunction({
      name: FUNCTION_NAME,
      data: Object.assign({ op }, payload || {}),
      success: (res) => {
        const result = res && res.result;
        if (!result || typeof result !== 'object') {
          reject(Object.assign(new Error('服务端没有返回结果'), { code: 'EMPTY' }));
          return;
        }
        if (!result.ok) {
          reject(Object.assign(new Error(result.message || '操作没有被接受'), { code: result.code }));
          return;
        }
        resolve(result);
      },
      fail: (error) => {
        reject(Object.assign(new Error(describe(error)), { code: 'NETWORK' }));
      },
    });
  });
}

module.exports = { call, describe, FUNCTION_NAME };
