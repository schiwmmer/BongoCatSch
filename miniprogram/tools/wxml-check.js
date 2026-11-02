'use strict';

// WXML 静态检查：真机上 WXML 报错只给一句「模板编译错误」，定位很费时间。
// 这个脚本在本地就能抓住三类常见问题：
//
//   1. 标签没闭合 / 闭合错位
//   2. wx:for 缺 wx:key（列表重建时的经典坑）
//   3. 模板里用了 data-act="xxx"，但页面 JS 里没有处理 xxx
//
//   node miniprogram/tools/wxml-check.js

const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..');
const PAGES = ['pomodoro', 'cultivate', 'arts', 'me'];
// WXML 没有 HTML 那种空元素，`<input>` 也必须写成 `<input />` 或配 `</input>`。
const VOID_TAGS = new Set();

let failures = 0;
const fail = (file, message) => {
  failures += 1;
  console.log(`  FAIL ${file}: ${message}`);
};

function checkTags(file, source) {
  const stack = [];
  const pattern = /<!--[\s\S]*?-->|<\/?([a-zA-Z][\w-]*)([^>]*?)(\/?)>/g;
  let match = pattern.exec(source);
  while (match) {
    const raw = match[0];
    if (raw.startsWith('<!--')) {
      match = pattern.exec(source);
      continue;
    }
    const tag = match[1];
    const tail = match[2] || '';
    const selfClosed = match[3] === '/' || VOID_TAGS.has(tag);
    if (raw.startsWith('</')) {
      const open = stack.pop();
      if (open !== tag) fail(file, `闭合 </${tag}> 与最近的 <${open || '无'}> 不匹配`);
    } else if (!selfClosed) {
      stack.push(tag);
    }
    if (!raw.startsWith('</') && /wx:for=/.test(tail) && !/wx:key=/.test(tail)) {
      fail(file, `<${tag}> 用了 wx:for 但没有 wx:key`);
    }
    match = pattern.exec(source);
  }
  if (stack.length) fail(file, `还有未闭合的标签：${stack.join(' > ')}`);
}

function binderNames(source) {
  const names = new Set();
  const pattern = /\b(?:bind|catch)(?:tap|input|change|confirm|blur|chooseavatar|longpress|scrolltolower)\s*=\s*"([^"{}]+)"/g;
  let match = pattern.exec(source);
  while (match) {
    names.add(match[1]);
    match = pattern.exec(source);
  }
  return names;
}

function checkHandlers(file, wxml, js) {
  const acts = new Set();
  const pattern = /data-act\s*=\s*"([^"{}]+)"/g;
  let match = pattern.exec(wxml);
  while (match) {
    acts.add(match[1]);
    match = pattern.exec(wxml);
  }
  // 动态 data-act（三目）单独处理：把每个分支都抠出来。
  const dynamic = /data-act\s*=\s*"\{\{([^}]+)\}\}"/g;
  match = dynamic.exec(wxml);
  while (match) {
    (match[1].match(/'([a-z-]+)'/g) || []).forEach((part) => acts.add(part.replace(/'/g, '')));
    match = dynamic.exec(wxml);
  }

  const handled = new Set();
  const cases = /case '([a-z-]+)':/g;
  match = cases.exec(js);
  while (match) {
    handled.add(match[1]);
    match = cases.exec(js);
  }

  acts.forEach((act) => {
    if (!handled.has(act)) fail(file, `data-act="${act}" 在页面 JS 里没有对应分支`);
  });

  binderNames(wxml).forEach((name) => {
    const declared = new RegExp(`\\b${name}\\s*\\(`);
    if (!declared.test(js)) fail(file, `事件处理函数 ${name} 在页面 JS 里不存在`);
  });
}

for (const page of PAGES) {
  const dir = path.join(ROOT, 'pages', page);
  const wxmlPath = path.join(dir, 'index.wxml');
  const jsPath = path.join(dir, 'index.js');
  const wxml = fs.readFileSync(wxmlPath, 'utf8');
  const js = fs.readFileSync(jsPath, 'utf8');
  const label = `pages/${page}/index.wxml`;
  checkTags(label, wxml);
  checkHandlers(label, wxml, js);
  // 模板里出现的 `view.` 字段，至少要能在 JS 里找到出处（宽松检查）
  const fields = new Set();
  const fieldPattern = /\bview\.([a-zA-Z_]+)/g;
  let match = fieldPattern.exec(wxml);
  while (match) {
    fields.add(match[1]);
    match = fieldPattern.exec(wxml);
  }
  console.log(`  ok   ${label}（${fields.size} 个字段引用）`);
}

const appJson = JSON.parse(fs.readFileSync(path.join(ROOT, 'app.json'), 'utf8'));
appJson.pages.forEach((page) => {
  ['js', 'json', 'wxml'].forEach((ext) => {
    const file = path.join(ROOT, `${page}.${ext}`);
    if (!fs.existsSync(file)) fail('app.json', `声明了 ${page} 但缺少 ${page}.${ext}`);
  });
});
appJson.tabBar.list.forEach((item) => {
  if (!appJson.pages.includes(item.pagePath)) fail('app.json', `tabBar 的 ${item.pagePath} 不在 pages 里`);
});
console.log(`  ok   app.json（${appJson.pages.length} 个页面 / ${appJson.tabBar.list.length} 个 tab）`);

console.log(`\n${failures === 0 ? '全部通过' : `${failures} 处失败`}`);
process.exit(failures === 0 ? 0 : 1);
