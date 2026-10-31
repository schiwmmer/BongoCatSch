# 番茄钟网站（浏览器版）

BongoCat 的番茄钟 + 修仙体系，做成一个网页。数据存在云端，换设备也在。

## 它和桌面版是什么关系

**同一份逻辑，不是重写。** 网页里跑的是 `crates/bongocat-pomodoro` 本身，编译成
`wasm32-unknown-unknown`，通过 `crates/bongocat-pomodoro-web` 的十来个
`extern "C"` 函数驱动。桌面上那个 25 分钟如何折成成长、一次突破要多少战力、
集市的种子怎么摇，全部是同一批代码，所以两边的数值不会各自漂移。

桌面版用 worker 线程 + 文件落盘，网页版没有这两样，替换为：

| 桌面 | 网页 |
| --- | --- |
| `PomodoroService` 的后台线程 | 页面每 250ms 调一次 `web_tick` |
| `pomodoro-tasks.json` | 云端 `pomodoro_saves` 表，每用户一行 |
| 系统时钟 `SystemTime::now()` | 页面传入的 `Date.now()/1000` |
| 首次出生潜质读时钟 | 页面传入随机种子（`crypto.getRandomValues`） |
| 每个交易日自动建「股票复盘」任务 | **不建**：`with_daily_stock_tasks(false)` |

## 数据与登录

存档是一个 JSON 文档（与桌面 `pomodoro-tasks.json` 同构），整条存在云端：

| 项 | 值 |
| --- | --- |
| 表 | `pomodoro_saves` |
| 列 | `owner_id TEXT PRIMARY KEY DEFAULT auth.uid()`、`document JSONB`、`saved_at TIMESTAMPTZ` |
| 隔离 | RLS：四类操作都要求 `owner_id = auth.uid()` |
| 登录 | 邮箱验证码（OTP），新用户验证后即注册，不设密码 |

前端从不发送 `owner_id`：列默认值填、RLS 策略校验。写入用 upsert，读用
`maybeSingle()`。

每次操作后 700ms 防抖写云端；失败会明确提示「未同步」并在 15 秒后重试，
不会静默当作保存成功。`localStorage` 只作**离线读取缓存**（断网冷启动时页面能开），
不是数据源；首次登录时若云端还没有这个账号的档，会把本机缓存上传一次。

## 构建与运行

```sh
just web          # 编译 wasm 并复制到 web/
just web-serve    # 编译并起 http://127.0.0.1:8099
just web-sdk      # 更新 vendor 里的云 SDK
```

`file://` 直接打开不行：页面要用 `fetch` 取 wasm 模块，浏览器的同源策略会拦下来。
把 `web/` 整个目录发布到任意静态托管即可。

`just web` 里的 `RUSTFLAGS='--cfg getrandom_backend="unsupported"'` 是必须的：
`atomic-write-file` 会带进 `rand`，而它在 wasm 上没有默认后端。网页构建从不写
文件，所以这个后端只需要"存在"，不需要"能用"。

## 文件

| 文件 | 作用 |
| --- | --- |
| `index.html` | 骨架：顶栏、侧栏、主区、登录层 |
| `styles.css` | 全部样式 |
| `wasm.js` | wasm 载入与调用封装 |
| `cloud.js` | 云 SDK 客户端：登录、读档、写档、离线缓存 |
| `app.js` | 外壳：会话守卫、导航、番茄钟页、日历、时钟循环 |
| `cultivate.js` | 修仙各页：状态、装备、功法、灵草园、集市、外出、塔、洞府 |
| `vendor/workbuddy-cloud-sdk.js` | 云 SDK 本地副本（见下） |
| `bongocat_pomodoro_web.wasm` | 构建产物，不入库 |

SDK 不走 CDN：jsdelivr 在国内网络不可达，登录页加载不到 SDK 就是白屏。
`just web-sdk` 会经本机 7890 代理拉 `@dev` 通道的最新构建覆盖本地副本——
不要把它固定成某个版本，需要更新时就跑一次。
