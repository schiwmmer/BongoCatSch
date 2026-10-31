# 番茄钟网站（浏览器版）

BongoCat 的番茄钟 + 修仙体系，做成一个纯静态网页。

## 它和桌面版是什么关系

**同一份逻辑，不是重写。** 网页里跑的是 `crates/bongocat-pomodoro` 本身，编译成
`wasm32-unknown-unknown`，通过 `crates/bongocat-pomodoro-web` 的十来个
`extern "C"` 函数驱动。桌面上那个 25 分钟如何折成成长、一次突破要多少战力、
集市的种子怎么摇，全部是同一批代码，所以两边的数值不会各自漂移。

桌面版用 worker 线程 + 文件落盘，网页版没有这两样，替换为：

| 桌面 | 网页 |
| --- | --- |
| `PomodoroService` 的后台线程 | 页面每 250ms 调一次 `web_tick` |
| `pomodoro-tasks.json` | `localStorage` 里的一条 JSON 字符串 |
| 系统时钟 `SystemTime::now()` | 页面传入的 `Date.now()/1000` |
| 首次出生潜质读时钟 | 页面传入随机种子（`crypto.getRandomValues`） |

## 去掉了什么

**键盘与鼠标活动计数。** 桌面版有一条 `PomodoroCommand::RecordInput`，把原生的
敲键/点击折算成修为。浏览器页面没有全局输入钩子，也没有理由为此申请权限，
所以命令解析层根本不接受这个 op —— 修为只来自完成的番茄。

## 构建与运行

```sh
just web          # 编译 wasm 并复制到 web/
just web-serve    # 编译并起 http://127.0.0.1:8099
```

`file://` 直接打开不行：页面要用 `fetch` 取 wasm 模块，浏览器的同源策略会拦下来。
任何静态服务器都可以，把 `web/` 目录整个发布出去即可。

`just web` 里的 `RUSTFLAGS='--cfg getrandom_backend="unsupported"'` 是必须的：
`atomic-write-file` 会带进 `rand`，而它在 wasm 上没有默认后端。网页构建从不写
文件，所以这个后端只需要"存在"，不需要"能用"。

## 文件

| 文件 | 作用 |
| --- | --- |
| `index.html` | 骨架：顶栏、侧栏、主区 |
| `styles.css` | 全部样式 |
| `wasm.js` | wasm 载入与 `localStorage` 存档 |
| `app.js` | 外壳：导航、番茄钟页、日历、时钟循环 |
| `cultivate.js` | 修仙各页：状态、装备、功法、灵草园、集市、外出、塔、洞府 |
| `bongocat_pomodoro_web.wasm` | 构建产物，不入库 |

## 存档

一条 JSON，键 `bongocat.pomodoro.web.v1`，格式就是桌面版的
`pomodoro-tasks.json`。清理浏览器数据即清档；换浏览器不通用。

离线时间会被结算：重开页面时，关着的那段时间按桌面同样的规则计入
（包括修仙的离线惩罚）。
