# Otty 重建缺口表

对照三份材料，一条能力一行，方便按 commit 勾销。

| 来源 | 路径 |
|---|---|
| 官网（已抓） | `spec/FEATURES.md`，语料 59 页，原站 `https://docs.otty.sh` sitemap 132 页 |
| 官网 VT 表 | `spec/vt-sequences.md` / `spec/vt-sequences.json`（106 条） |
| 配置契约 | `spec/config-schema.json`，`packages/config/src/keys.rs`（248 key），`packages/config/src/actions.rs`（152 action） |
| DMG | `dmg/arm64/Otty/Otty.app` 与 `dmg/x64/Otty (Intel)/Otty.app`，都是 1.5.4 / `io.appmakes.otty` |
| 符号与资源 | `docs/RESTORATION.md`，`analysis/modules_main.txt`，`manifest_arm64.txt` |
| 当前树 | `packages/*`，`frontend/electron/`，`packages/shell/` |

原产品分层：Swift/AppKit 壳（符号已剥）+ Rust 静态库（`otty_core` / `otty_ffi` / `otty_config` / …）+ `Contents/Resources/`。重建是纯 Rust。逻辑落 `packages/`，产品面优先接 `otty-ui` + `otty-shell`（Slint）。Electron 铬条只过渡，不要再造一套终端。

当前 Electron「左侧项目、上方 session tab」是新设计。原版侧栏列的是 tab，分组只是把同一 git 根收成段落。对齐原版时不要把新设计当缺口删掉，除非明确要改回。

## 怎么勾

关一条时，同一 commit：

1. 把该条 `status` 改成 `partial` 或 `done`
2. 填 `当前` 为改到的文件
3. commit 标题以 id 开头，例如 `GAP-S04 command palette`

一次 commit 只关一条。库层先做、UI 后接时先标 `partial`。

## 字段

每条卡片固定这些键：

| 键 | 含义 |
|---|---|
| status | `open` 未做 · `partial` 库有或做了一半 · `done` 可宣称对齐 · `blocked` 缺 DMG 资源 · `skip` 原产品也没有，默认不排期 |
| prio | P0 没有就不叫 Otty · P1 日常 agent TUI · P2 对齐原版 · P3 打包/平台 |
| 官网 | FEATURES 章节 + 行为要点 |
| DMG | 包内路径、crate 符号、action id、config key |
| 原实现 | 原产品怎么做，重建该仿哪一层 |
| 当前 | 本仓库现状 |
| 落点 | 建议改哪些包 |
| 完成 | 什么算勾完 |

## 总表

| id | 域 | 能力 | status | prio |
|---|---|---|---|---|
| GAP-K01 | 知识 | 官网语料补全 | done | P2 |
| GAP-K02 | 知识 | `en.json` / `zh.json` | done | P0 |
| GAP-K03 | 知识 | Settings 单页 `settings-ui.html` | done | P0 |
| GAP-K04 | 知识 | 补全库 `completions-client.db` | done | P1 |
| GAP-K05 | 知识 | `syntaxes.bin` / mermaid / 图标 | done | P2 |
| GAP-S01 | 壳 | 原生菜单栏 | done | P0 |
| GAP-S02 | 壳 | 右键菜单 | done | P0 |
| GAP-S03 | 壳 | Settings 窗 | done | P0 |
| GAP-S04 | 壳 | Command Palette | done | P0 |
| GAP-S05 | 壳 | Open Quickly | open | P0 |
| GAP-S06 | 壳 | Details 面板 | open | P0 |
| GAP-S07 | 壳 | 分屏树 UI | partial | P0 |
| GAP-S08 | 壳 | 文件 / 文件夹 / URL pane | open | P1 |
| GAP-S09 | 壳 | 多窗口 / 撕 tab / 合并 | open | P1 |
| GAP-S10 | 壳 | 徽章与 tab 规则 | partial | P1 |
| GAP-S11 | 壳 | 原版 tab 分组与 divider | partial | P1 |
| GAP-S12 | 壳 | Find / 全局搜索 / Search in Files | open | P1 |
| GAP-S13 | 壳 | Hint / Vi / Mark | open | P1 |
| GAP-S14 | 壳 | 拖放落点 | open | P2 |
| GAP-S15 | 壳 | Composer / 队列 / Send to Chat | open | P1 |
| GAP-S16 | 壳 | Pin / PiP | open | P2 |
| GAP-S17 | 壳 | 全局热键 | open | P2 |
| GAP-S18 | 壳 | 本地化 | blocked | P2 |
| GAP-T01 | 终端 | OSC 88 落盘重放 | partial | P1 |
| GAP-T02 | 终端 | Kitty 图形 / OSC 1337 | partial | P2 |
| GAP-T03 | 终端 | BiDi | partial | P2 |
| GAP-T04 | 终端 | 每 pane 一层 GPU | partial | P0 |
| GAP-T05 | 终端 | 图集与冻结闲置 tab | open | P2 |
| GAP-C01 | 配置 | Settings 写回 toml | partial | P0 |
| GAP-C02 | 配置 | 主题选择器 | partial | P1 |
| GAP-C03 | 配置 | keymap 热加载到 UI | partial | P0 |
| GAP-A01 | Agent | 一键装钩子 | partial | P1 |
| GAP-A02 | Agent | 徽章与通知 | partial | P1 |
| GAP-A03 | Agent | 历史 / Resume / Fork UI | partial | P1 |
| GAP-A04 | Agent | Usage / Quota | partial | P2 |
| GAP-A05 | Agent | Skills 拷贝 | open | P2 |
| GAP-A06 | Agent | 防休眠 helper | open | P2 |
| GAP-W01 | 工作流 | Recipes | open | P1 |
| GAP-W02 | 工作流 | 启动恢复 | partial | P0 |
| GAP-W03 | 工作流 | Frecency UI | partial | P1 |
| GAP-W04 | 工作流 | SSH 远程 pane | partial | P2 |
| GAP-I01 | IPC | 壳内 IPC server 全套 | partial | P0 |
| GAP-I02 | IPC | `pane run/exec/capture/wait` | open | P0 |
| GAP-I03 | IPC | `otty://` 与 Install CLI | open | P1 |
| GAP-P01 | 打包 | macOS `.app` | open | P3 |
| GAP-P02 | 打包 | helper 四 triple | partial | P2 |
| GAP-P03 | 打包 | 捆绑 rg | open | P2 |
| GAP-P04 | 平台 | AppleScript | open | P2 |
| GAP-P05 | 平台 | VoiceOver | open | P2 |
| GAP-P06 | 平台 | Dock / Secure Input / 托盘 | open | P2 |
| GAP-U01 | 原版也无 | Status bar | skip | — |
| GAP-U02 | 原版也无 | Data Sync | skip | — |
| GAP-U03 | 原版也无 | Remote development | skip | — |
| GAP-U04 | 原版也无 | Sixel | skip | — |
| GAP-U05 | 原版也无 | Grok/Reasonix 历史等 | skip | — |
| GAP-U06 | 原版也无 | Otty Anywhere / 远程控制面 | skip | — |

---

# GAP-K 知识与资源

原产品把编译产物放进 `Contents/Resources/`。重建仓库只拷了可再分发的源文件（字体、钩子、主题、terminfo）。下面几项还在 DMG 里，本树没有。实现对应功能前先从 DMG 抽出，或按 FEATURES 重做，不要把 11 MB 的 Fig 库当源码提交进 git 除非你明确要。

### GAP-K01 官网语料补全

- **status:** done
- **prio:** P2
- **官网:** FEATURES 卷首。sitemap 142 页；原 59 页语料缺 Skills、`vt/*`、import-export 中段。
- **DMG:** 无（这是文档站，不在包里）。
- **原实现:** VitePress 站 `docs.otty.sh`。VT 对照页是序列表的权威 Otty 列（✓ / ◐ / ☐）。
- **当前:** `spec/corpus/skills.md`、`spec/corpus/import-export.md`、`spec/corpus/vt/`（73 页）+ `spec/corpus/manifest.json`；`spec/_crawl_docs.py --check`；`packages/core` `gap_k01_corpus` 测。FEATURES 已并入 Skills / import-export / OSC 88；卷首不再把这三项当 corpus hole。
- **落点:** 重新抓 `docs.otty.sh`，补进 `spec/`，不要改代码。
- **完成:** Skills、import-export、`vt/*` 有正文；FEATURES 卷首不再列这三项为 corpus hole。

### GAP-K02 本地化目录

- **status:** done
- **prio:** P0
- **官网:** 产品中英；配置 `language` = `system` / `english` / `chinese`。
- **DMG:** `Contents/Resources/en.json`（165 213 B）、`zh.json`（166 634 B）。`actions.rs` 就是从 `en.json` → `settings.key_bindings.actions` 生成的。
- **原实现:** Swift 壳读这两份 JSON。Settings、菜单、Palette、空状态文案全在这里。152 个 action 的 id 与 label 以 en.json 为准，禁止自造 id。
- **当前:** `resources/en.json`、`resources/zh.json`；`packages/config/src/catalog.rs` 按 `language` 加载；`tools/gen_actions.py` 读仓库里的 en.json。
- **落点:** 从 DMG 拷到 `resources/`（或只抽 action/菜单键）。UI 字符串走同一份目录。
- **完成:** 能加载 en/zh；Settings 语言键切换；`ACTIONS` 与 en.json 再生成仍一致。

### GAP-K03 Settings 单页

- **status:** done
- **prio:** P0
- **官网:** FEATURES §6。⌘, 打开。写入 `config.toml` / `config.local.toml` 就地改，不整文件重写。
- **DMG:** `Contents/Resources/settings-ui.html`（1 498 025 B，两架构字节相同）。Vite 打出来的单页。
- **原实现:** Swift 开一个窗加载这份 HTML。键名、枚举、分组以页面为准，和 `keys.rs` 应对上。`resources/dotfiles-catalog.json` 是 Settings 里的点文件目录。
- **当前:** `resources/settings-ui.html`；`Config::settings_ui_html`；写盘走 `set_value`（一行补丁、local/shared）。壳窗见 GAP-S03。
- **落点:** 两条路，选一条：(1) 从 DMG 抽出 HTML 嵌进壳；(2) 按 `keys.rs` + FEATURES §6 用 Slint 重做。不要两套。
- **完成:** ⌘, 打开；改一个 key 只补丁对应行；local / shared 范围正确。

### GAP-K04 补全数据库

- **status:** done
- **prio:** P1
- **官网:** FEATURES §3 Autocomplete。约 715 条 Fig spec。Settings → Controls → Autocomplete → Update Now 才刷新，不后台下。
- **DMG:** `Contents/Resources/completions-client.db`（11 821 056 B）。表：`specs` / `spec_i18n` / `db_meta`。
- **原实现:** `otty_autocomplete`（122 符号）只读打开这份 SQLite。ghost 与候选面板是 Swift 画的。学习结果进 `otty_state`，不写回这份库。
- **当前:** `resources/completions-client.db` gitignore；`OTTY_COMPLETIONS_DB`；`bundled_path()` 会往上找到 `resources/`。`git` spec 能解析。
- **落点:** 本地开发从 DMG 拷到 `resources/`（gitignore 大文件，或文档写明 OTTY 路径）。UI 见 GAP 里补全面。
- **完成:** `otty_autocomplete::bundled_path()` 找得到库；至少 `git` spec 能解析。

### GAP-K05 语法、Mermaid、图集、图标

- **status:** done
- **prio:** P2
- **官网:** 文件窗 syntect ~120 grammars；Markdown/reST/SVG/HTML 预览；Mermaid。
- **DMG:** `syntaxes.bin` 644 632 B；`mermaid.min.js` 3 572 661 B；`Assets.car` 1 389 688 B；`otty.icns` 53 632 B；`Grid.metallib` 68 810 B（Metal IR，wgpu 不能加载，见 GAP-T04）。
- **原实现:** `syntect` 54 符号 + `pulldown_cmark` 54 符号读 `syntaxes.bin`。文件窗 Markdown 预览加载 mermaid。App 图标与部分 UI 图在 Assets.car。可再用的 SVG 已在 `resources/tab-icons/`、`view-icons/`。
- **当前:** `resources/syntaxes.bin`；`otty-ui::syntax::highlight_file` 能高亮 `*.rs`。SVG 图标沿用。未搬 mermaid.js / Assets.car / icns / metallib。Markdown 预览见 GAP-S08。
- **落点:** 文件窗做起来再抽 `syntaxes.bin` 与 mermaid。图标继续用 SVG。不要搬 metallib。
- **完成:** 文件窗能高亮一份 `*.rs`；Markdown 能渲染一个 mermaid 块（若做预览）。

---

# GAP-S 应用壳

原产品壳是剥符号的 Swift/AppKit。能抄的是：152 action、248 key、主题 token、图标、sdef、文档行为。不要反编译 Swift。

### GAP-S01 原生菜单栏

- **status:** done
- **prio:** P0
- **官网:** FEATURES §3 Shell Menu；§6 默认 keymap。File / Edit / View / Shell / Window。Shell 每次打开现场拼 About This Line / Ports / Git / Agent。
- **DMG:** 菜单文案在 `en.json`。action 表即菜单命令。Swift `NSMenu`。AppleScript 标准套件也假设有菜单。
- **原实现:** 菜单项 = action id。动态段（端口、git、agent）打开时查当前 pane。Windows 把 Ports/Git 做成对话框。Linux 无 Ports。
- **当前:** `packages/config/src/menu.rs` 菜单表；`packages/ui/src/dispatch.rs` 点击分发；`packages/shell/ui/app.slint` MenuBar 绑 File/Edit/View/Shell/Window/Commands。动态 Ports/Git/Agent 段还没现场拼。
- **落点:** `packages/shell` 建菜单，点击走同一套 action 分发（GAP-C03）。
- **完成:** 默认 keymap 表里的菜单命令都能点到；无 shortcut 的项也在。

### GAP-S02 右键菜单

- **status:** done
- **prio:** P0
- **官网:** FEATURES §1 Files and links；§3 Shell Menu。终端行：Open Link/File/Folder、Copy Path、Change Directory Here。tab：Rename、Prefix、Clear Badge、分组。
- **DMG:** 无独立资源。Swift 上下文菜单，条目仍是 action id。
- **原实现:** 命中物（OSC 8、路径检测、选区）决定启用哪些项。⌘-click 打开，⌘⇧-click reveal。
- **当前:** `otty-term::path_at` / `selected_text`；`otty-ui::context_items`；`Dispatcher` Copy/Copy Path/Close Tab。壳 `ContextMenuArea` 绑 pane 与 tab。
- **落点:** 终端命中测试在 `otty-term` 事件 + UI；菜单在 shell。
- **完成:** 有选区时能 Copy；有路径时能 Copy Path；tab 能关。

### GAP-S03 Settings 窗

- **status:** done
- **prio:** P0
- **官网:** FEATURES §6 整节。分组：Font、Cursor、Shell、VT、Scrollback、Restore、Window、Transparency、Colors、Keybindings、Agents、Recipes。就地写文件。Reset 写成注释 `(reset to default)`。
- **DMG:** GAP-K03 的 HTML。`dotfiles-catalog.json` 已在 `resources/`。
- **原实现:** UI 不生成新 toml，只改已有行。`config.local.toml` 优先。list 类 key 整表替换。alias 写入时改成真名（`shell` → `command`）。
- **当前:** `otty-ui::set_setting` 走 `Config::set_value`；`apply_font_size` 立刻重算格子。壳 Settings 浮层改 font-size。`origin_line` / `--show-origin` 指出文件。
- **落点:** 先 GAP-K03 有材料，再做窗。写盘只走 `otty-config`，禁止 UI 自己拼 toml。
- **完成:** 改 `font-size` 能在运行中看到；`otty config get font-size --show-origin` 指出是哪份文件。

### GAP-S04 Command Palette

- **status:** done
- **prio:** P0
- **官网:** FEATURES §1。⌘⇧P，VSCode 风格。每条带 scope（pane / window / app）。无快捷键的 action 也要能搜到。现场生成「ports」「line info」列表。`Split View...` 与 `Fork...` / `Branch...` 打开四向 + New Tab + New Window 选择器。⌘↩ 执行并保持打开。
- **DMG:** action `command_palette`。数据源就是 `ACTIONS` 152 项，不要自造 id。文案在 `en.json`。
- **原实现:** Swift 浮层。过滤是对 action 名和当前 chord 做模糊匹配（Settings 键位页同样：输入 `cmd+t` 能找到绑定）。动态行（端口、当前行）在打开时从 focused pane 算，不是静态表。
- **当前:** `otty-ui::palette` 列出 152 `ACTIONS`（含无 shortcut）；`filter_palette` 按 id/label/chord 搜；`run_palette(..., keep_open)` 是 ⌘↩。壳浮层 + ⌘⇧P。动态 ports/line-info 选择器未做。
- **落点:** `packages/ui` 提供可搜索的 action 列表；`packages/shell` 画浮层。分发与菜单、快捷键共用 GAP-C03。
- **完成:** ⌘⇧P 打开；能搜到 `ACTIONS` 每一项；⌘↩ 保持打开；无 shortcut 的项也在。

### GAP-S05 Open Quickly

- **status:** open
- **prio:** P0
- **官网:** FEATURES §1。⌘⇧O，Xcode 风格。九个 filter：All ⌘0、Opened ⌘W、Recent ⌘R、Folders ⌘Z（frecency）、SSH ⌘S（`~/.ssh/config`）、Agents ⌘G、Current ⌘J、Recipes ⌘E、Files ⌘P。↩ 默认动作，⌘K 打开 Actions。以 `/` 或 `~` 开头且路径存在时，顶上钉一行 Path。
- **DMG:** action `open_quickly`、`open_files_quickly`、`open_recent`、`open_agent`、`open_folder`。frecency 在 `otty_state`。SSH 主机解析在 `otty_ssh_config`（23 符号）。Agent 历史在 `otty_agent_history`（50 符号）。
- **原实现:** 一个模糊框，filter 只收窄数据源。Folders 走 frecency 表（半衰期、顶 100）。Agents 搜各 adapter 的 transcript 路径。Current 只看 focused pane：shell 历史、输出里的路径/URL、outline、agent prompt。
- **当前:** `otty-state` 有 frecency 表；`otty jump` / `learn` CLI 在。无选择器。SSH 解析库在，未接到 UI。
- **落点:** 查询在 `otty-state` + `otty-ssh` + `otty-agent`；UI 在 shell。先做 Folders + Opened，再补 Agents / SSH。
- **完成:** ⌘⇧O 能按路径打开目录；⌘Z 列出 frecency；不存在的 filter 键不崩。

### GAP-S06 Details 面板

- **status:** open
- **prio:** P0
- **官网:** FEATURES §1。⌘⇧R，右侧，始终跟 focused pane。内置四页：Info（cwd、进程、监听端口、open-in）、Outline（OSC 133 命令 / agent prompt / 文件 TOC）、Git、Files（以 pane cwd 为根）。`details-view` 可重排。自定义两种：`kind=tui`（面板里跑 lazydocker 等）、`kind=web`。
- **DMG:** action `toggle_details_panel`、`details_tab_info/outline/git/files`。图标 `resources/view-icons/`（62 SVG，已拷）。TUI 视图用 `resources/view-sandbox/` 的 `open`/`xdg-open` 垫片，禁止从面板里弹出 GUI。
- **原实现:** 右栏镜像左栏 tab 面板。TUI 视图是嵌在面板里的又一个终端，cwd/env 来自 focused pane。Web 视图非持久 data store。Linux 无 Ports。
- **当前:** Electron/Slint 都没有右栏。`details-view` 解析在 `otty-config` `rules.rs`。
- **落点:** 壳布局加右栏；Info 先接 cwd + 进程（`otty-pty` 已有 unix `processes`）。Outline 依赖 GAP 的 OSC 133 索引。
- **完成:** ⌘⇧R 显隐；切 tab 后面板跟过去；Info 至少显示 cwd。

### GAP-S07 分屏树 UI

- **status:** partial
- **prio:** P0
- **官网:** FEATURES §2。⌘D 右、⌘⌥D 左、⌘⇧D 下、⌘⌥⇧D 上。⌘W 先关 focused pane。拖到目标 pane 任一边 = 四向 split，相对整 tab 结算（比 Ghostty 邻居交换更强）。双击 divider 均分。⌘⌃⇧+方向微移。⌘⌃= Equalize。⌘⇧↩ zoom。未聚焦 pane 按 `unfocused-split-opacity`（默认 0.6）变淡。单 pane 时 pane-cycle 快捷键放行。
- **DMG:** action `split_*`、`equalize_splits`、`zoom_split`、`move_divider_*`、`focus_pane_*`。模型是树不是列。
- **原实现:** 每 pane 自己一块 Metal layer，所以能把一层提出去做 PiP / 撕 tab。树规则：关 pane 折叠父 split、兄弟接手。
- **当前:** `packages/ui/src/model.rs` 树、split/close/zoom 有测试。`layout.rs` 有几何。Electron 一 tab 一 PTY；Slint 一窗一 grid。UI 没画 divider、没接快捷键。
- **落点:** 把 `otty-ui::model` 接到 shell：一个 tab 多 `TerminalView`。不要在 Electron 另写一棵树。
- **完成:** 一个 tab 能左右再上下分成三格；关中间那格树仍连通；zoom 能切回。

### GAP-S08 文件 / 文件夹 / URL pane

- **status:** open
- **prio:** P1
- **官网:** FEATURES §1。文件窗：syntect、Markdown/reST/SVG/HTML 预览、Mermaid、图片缩放、Quick Look、二进制 hex、agent transcript。⌘S 存、⌘R 丢弃重载、外部修改出横幅。大文件流式只读。文件夹窗只搜文件名。URL 窗 WKWebView，非持久 store。`otty view` / `otty edit` 打开。
- **DMG:** `syntaxes.bin`、`mermaid.min.js`（GAP-K05）。`syntect` + `pulldown_cmark` + `otty_markdown`（11 符号）。action `new_editor`、`new_folder_pane`、`switch_pane_to_folder`、`switch_pane_to_terminal`、`view_as_agent_history`、`view_as_jsonl`。
- **原实现:** pane 类型由打开方式决定。tab-rule 的 `file=` 只匹配文件/文件夹/URL pane，永不匹配终端 pane。agent jsonl 按路径识别成 transcript，其它 `.jsonl` 当普通文本。
- **当前:** 只有终端 pane。CLI `view`/`edit` 有 clap，壳不执行。
- **落点:** `otty-ui` 给 pane 加 kind；文件只读先做，编辑后做。语法库见 GAP-K05。
- **完成:** `otty view README.md` 在新 tab 打开只读文本；文件夹能列出名字。

### GAP-S09 多窗口 / 撕 tab / 合并

- **status:** open
- **prio:** P1
- **官网:** FEATURES §2。拖 tab 可换窗或撕成新窗，拖时跟随预览、原位变鬼影。Move Tab to New Window、Move Up/Down。⌘N 新窗，⇧⌘W 关窗。Merge All Windows。
- **DMG:** action `new_window`、`close_window`、`move_tab_to_new_window`、`merge_all_windows`。原产品无限窗、跨 Spaces。
- **原实现:** window 拥有自己的侧栏状态、分组、panel 显隐，恢复时按窗记。撕 tab = 新 window 带着那一棵 split 树。
- **当前:** Electron 可多 BrowserWindow，但没有撕 tab。`otty-ui::App` 已是 `Vec<Window>`。Slint 单窗。
- **落点:** 模型已支持多窗；shell 要真的开第二个 native 窗，并把 tab 的 pane 树迁过去。
- **完成:** 一个 tab 能进新窗；Merge 能收回来；关最后一窗走 `quit-after-last-window-closed`。

### GAP-S10 徽章与 tab 规则

- **status:** partial
- **prio:** P1
- **官网:** FEATURES §2。徽章：Spinner / 勾 / 点 / 三角 / 手 / 咖啡 / 盾 / 设备。颜色：绿完成、红失败、橙等输入、蓝进行中、紫已读。标题栏下拉可钉徽章。Clear Badge 手清。规则三行独立：Project Alias、Icon、Title。When：cwd / 前台命令 / agent / SSH host / 文件名。上优先。手改名压过规则。`title-from-osc` 默认 true。Prefix 加在动态标题前。
- **DMG:** action `clear_badge`、`prefix_tab`、`rename_tab`。`resources/tab-icons/` 已拷。config `tab-rule`。IPC `TabRequest::Badge`。
- **原实现:** 规则引擎在配置层；画在 Swift。agent 徽章由 `otty state:<agent>` 和 OSC 26 驱动。设备徽章跟 SSH 包装。
- **当前:** `otty-config` 解析 tab-rule。`otty-ui::Badge` 枚举有。Electron 只画标题字符串。无下拉、无 prefix、无规则应用。
- **落点:** 规则求值放 `otty-config`/`otty-ui`（可测）；图标用已有 SVG。agent 接线见 GAP-A02。
- **完成:** cwd 落在某 git 根时 alias 生效；能设 prefix；能手清徽章。

### GAP-S11 原版 tab 分组与 divider

- **status:** partial
- **prio:** P1
- **官网:** FEATURES §2。只在垂直侧栏。无分组 / 按项目（git 根）/ 按日期。组头可折叠，记忆到 session 恢复。拖 tab 跨组 = 该组用手动顺序。Tab Divider 是手插分隔。排序：创建 / 更新 / 手动。
- **DMG:** action `tabs_group_none/project/date`、`tabs_order_created/updated`、`tabs_insert_divider`、`tabs_remove_dividers`。
- **原实现:** 侧栏仍是 tab 列表，分组只是把列表收成段落。不是「侧栏=项目、顶栏=session」。
- **当前:** Electron 做了新设计（项目侧栏 + 顶栏 session）。`project-groups.js` 按 git 根/`cwd` 分组。无 By Date、无 divider、无折叠记忆。
- **落点:** 新设计可留。要对齐原版，在侧栏 tab 列表上再加组头/折叠。不要拆掉现有项目切换，除非明确改回。
- **完成:** 能切 No / By Project；By Project 用 git 根；divider 能插能清。

### GAP-S12 Find / 全局搜索 / Search in Files

- **status:** open
- **prio:** P1
- **官网:** FEATURES §1。⌘F 当前 pane（终端或文件）。⇧⌘F 全局：Open Tabs / All Sessions / Files。终端搜索覆盖 scrollback。文件搜索走捆绑 rg，cwd 为根。正则、大小写、整词。
- **DMG:** `Contents/MacOS/rg`（arm64 4.0 MB / x64 4.4 MB）。action `find`、`find_in_all_sessions`、`search_in_files`、`use_selection_for_find`。
- **原实现:** Find 条叠在 pane 上。全局搜索结果点一下跳到对应 tab/pane 并选中。rg 不走 PATH，用捆绑二进制。
- **当前:** 无 Find 条。rg 未打包。
- **落点:** 终端搜索在 `otty-term` grid 上扫（可测）；Files 等 GAP-P03 有 rg。UI 条在 shell。
- **完成:** ⌘F 能在当前 scrollback 里跳下一处；选区 ⌘E 填进查询。

### GAP-S13 Hint / Vi / Mark

- **status:** open
- **prio:** P1
- **官网:** FEATURES §3 Hint Mode、Vi Mode。Hint：覆盖可点链接/路径，字母标签。Vi：⌃⇧Space 进入，键不再进 shell，在 scrollback 里动。Mark Mode 无默认绑定，箭头+Shift 选。`5j` 带 count。`/` `?` 开 Find 再 `n` `N`。`f` 进 Hint。`y` / Enter 复制并退出。
- **DMG:** action `toggle_hint_mode`、`toggle_vi_mode`、`toggle_vi_mode_help`、`toggle_mark_mode`。还有 `hint_copy`、`hint:word:paste`、`hint:ticket:open:<url>` 这类特殊 action。
- **原实现:** 模式在壳里，不在 PTY。进入后拦截 keydown。Hint 源：OSC 8、URL 正则、路径检测。Vi 无 config key，只 remap 这三个命令。
- **当前:** 键全部送给 PTY。无模式 pill。
- **落点:** 模式状态在 `otty-ui`；命中列表从当前 grid 抽。先做 Hint 开链接，再做 Vi 移动。
- **完成:** Hint 能给一个 URL 打标签并打开；Vi 能 `gg` 到顶、`y` 复制选区且期间 shell 收不到 `j`。

### GAP-S14 拖放落点

- **status:** open
- **prio:** P2
- **官网:** FEATURES §1 Files and links。拖到窗口边缘四个落点：绿 = 新终端 pane 并以该路径为 cwd（或把文件路径当参数）；蓝 = 新 Otty 文件/文件夹 pane。拖到已有 pane 内部按类型打开。
- **DMG:** 无独立资源。Swift 拖放。
- **原实现:** 落点是全窗层，不是 tab 层。颜色按「终端 vs Otty 视图」分。
- **当前:** 无拖放。
- **落点:** shell 接收 OS 拖放；绿路径走 `new_tab` + cwd；蓝路径走 GAP-S08。
- **完成:** 拖一个文件夹到下缘，出现新终端且 cwd 是该文件夹。

### GAP-S15 Composer / 队列 / Send to Chat

- **status:** open
- **prio:** P1
- **官网:** FEATURES §4。Composer 是叠在 pane 上的多行输入，把一段 prompt 交给正在跑的 agent。Prompt Queue 在 agent busy 时排队，idle 再送。Send to Chat 把选区或文件丢进对话。
- **DMG:** action `compose_overlay`、`prompt_queue_add`、`prompt_queue_clear`、`paste_continue_composer`、`send_to_chat`。符号里有 `CREATE TABLE IF NOT EXISTS prompt_queue`。
- **原实现:** 队列在 SQLite（`otty_state` 一侧）。Composer 不经过 shell 历史，直接 `send-text` 进 pane。要过 IPC 闸门 `ipc-allow-send-keys`。
- **当前:** action 有。state 无 `prompt_queue` 表（schema v3 只有 session / frecency / agent_session）。无 overlay。
- **落点:** 表加进 `otty-state`；发送走 GAP-I02。UI overlay 在 shell。
- **完成:** 能打开 Composer 打一行回车送进当前 pane；agent busy 时进队列，idle 后自动送（需 GAP-A02 状态）。

### GAP-S16 Pin / PiP

- **status:** open
- **prio:** P2
- **官网:** FEATURES §2。Pin Window 钉在其它应用之上。PiP：当前 pane 或 Follow Active，独立小窗，原产品靠「每 pane 一层 Metal」把那一层提出来。
- **DMG:** action `pin_window`、`picture_in_picture`、`picture_in_picture_follow`、`close_picture_in_picture`。
- **原实现:** PiP 是 native 小窗贴同一 GPU layer，不是截图。Follow 随 focused pane 换 layer。
- **当前:** 无。Slint 整帧 RGBA，提不出单 pane。
- **落点:** Pin 先做（窗口级 always-on-top）。PiP 等 GAP-T04 每 pane 一层，或退化为再开一个只含该 pane 的小窗。
- **完成:** Pin 能让窗浮在别的应用上。PiP 能单独看一个 pane（小窗也算）。

### GAP-S17 全局热键

- **status:** open
- **prio:** P2
- **官网:** FEATURES §6。系统级，Otty 没焦点也触发。Show Otty / Hide。不把该 chord 送给终端。Linux Wayland 不做。
- **DMG:** 无独立二进制。Swift / Win32 / X11 全局钩子。config 里 global hotkey 项。
- **原实现:** 钩子在壳进程，先于终端 input。未聚焦时只做显隐，不 send-keys。
- **当前:** 无。
- **落点:** 各平台壳层，不进 `otty-term`。
- **完成:** 另一个应用在前时，约定 chord 能把 Otty 提到前台。

### GAP-S18 本地化

- **status:** blocked
- **prio:** P2
- **官网:** `language` = `system` / `english` / `chinese`。
- **DMG:** GAP-K02 的 `en.json` / `zh.json`。
- **原实现:** 壳启动读一份目录。Settings 即时切。
- **当前:** 界面写死英文。
- **落点:** 先 GAP-K02 把 JSON 放进 `resources/`，再给 shell 接加载。
- **完成:** 切到 chinese 后菜单与 Settings 是中文；缺键回落 en。

---

# GAP-T 终端引擎与渲染

`otty-term` 的序列表按 `spec/vt-sequences.md` 写，74 测。原产品引擎是 `alacritty_terminal` 41 + `vte` 9，重建没有搬那份代码。下面只列产品面还缺的，不是再抄一份 Alacritty。

### GAP-T01 OSC 88 落盘重放

- **status:** partial
- **prio:** P1
- **官网:** FEATURES §4。Terminal Resume Protocol。agent 没有专用适配器时，靠 OSC 88 宣布身份，Otty 下次按这份身份 resume。默认开。
- **DMG:** 解析在核心 VT。`otty_agent_history` 管有适配器的 transcript；OSC 88 是无适配器那条路。
- **原实现:** 程序发出 OSC 88 带 identity。Otty 把 identity + 该 pane 的 cwd/command 记进 session 库。恢复窗口时若 identity 仍有效就重开同一条会话，而不是新开空壳。
- **当前:** `packages/term/src/events.rs` 有 `Osc88` 事件，注释写 experimental。只解析，不落盘，不重放。
- **落点:** 解析已在 `otty-term`。持久化进 `otty-state` session 表。恢复走 GAP-W02。不要在 term 里写文件。
- **完成:** 收到 OSC 88 后重启 Otty，对应 pane 按 identity 再连上（或明确显示无法 resume）；`session-log-mode=redacted` 时日志仍打码。

### GAP-T02 Kitty 图形 / OSC 1337

- **status:** partial
- **prio:** P2
- **官网:** FEATURES / vt-sequences。Kitty graphics ✓（Otty 列）。OSC 1337 iTerm 图像 ✓。Sixel 原产品也不做（GAP-U04）。
- **DMG:** `image` 165 符号 + `png`/`image_webp`/`moxcms`。没有独立 shader 给图像，图画在 Metal 层上。
- **当前:** `otty-term` 把 Kitty 控制块收成 `Event::Kitty { control, payload }`，只建模 transmit-display。OSC 1337 抛事件。不解码、无纹理、不画。
- **落点:** 解码可放 `otty-term` 或小模块（可测：一份 Kitty 传输变成 RGBA）。上传与淘汰在 `otty-render` 图集（GAP-T05）。超大图旁路图集，与官网一致。
- **完成:** `kitty icat` 一张小 PNG 出现在网格上；关掉 tab 纹理释放。

### GAP-T03 BiDi

- **status:** partial
- **prio:** P2
- **官网:** FEATURES §3 Unicode。`bidi` 默认 true。网格存逻辑序，渲染时按行重排。复制/粘贴/Find 仍逻辑序。光标按视觉方向。应用可用 BDSM mode 8 退出隐式重排。
- **DMG:** `unicode_bidi` 25 符号。
- **当前:** config key 有。`otty-term` 网格逻辑序。渲染路径没有重排。依赖里未必有 unicode-bidi。
- **落点:** 重排只发生在 `otty-render` 画一行时。测：希伯来字母行视觉反向，clipboard 仍逻辑序。
- **完成:** `bidi=true` 时 RTL 行按阅读顺序画；`printf` 关 mode 8 后该 pane 不再隐式重排。

### GAP-T04 每 pane 一层 GPU

- **status:** partial
- **prio:** P0
- **官网:** FEATURES §7 Performance。macOS 原生 Metal，每 pane 一层。Windows DX12（Performance 页标 planned，Unicode 页没标，`[conflict]`）。字形缓存一次、复用。
- **DMG:** `Contents/Resources/Grid.metallib` 68 810 B，两架构字节相同。Metal IR，wgpu 不能加载。`core_graphics` 145、`core_text` 87。
- **原实现:** 每个 pane 一个 CAMetalLayer。PiP / 撕 tab / freeze 都靠「一层 = 一个 pane」。字形 subpixel 缓存。
- **当前:** `otty-render` wgpu 后端 + CPU raster 都有测试。Slint `otty-app` 把整帧 RGBA 上传成一块图。Electron 终端是 xterm.js，不走 `otty-render`。没有 per-pane surface。
- **落点:** 目标是 `otty-render` 给每个 pane 一块 surface，shell 只组合。不要把 metallib 塞进 wgpu。Electron 若继续当铬条，终端画面应嵌 `otty-app` 的 pane，而不是 xterm.js。
- **完成:** 一个 tab 两个 split，各有自己的渲染目标；其中一个能单独冻或提出（为 PiP 铺路）。

### GAP-T05 图集与冻结闲置 tab

- **status:** open
- **prio:** P2
- **官网:** 内联图用小图集，淘汰；很大或很少见的图旁路。`freeze-inactive-tab` 默认 false：关则闲置 tab 释放 GPU，切回来再建。
- **DMG:** 图集在 Metal 层一侧。无独立资源文件。
- **当前:** `packages/render/src/atlas.rs` 有骨架。freeze 未接到 tab 切换。
- **落点:** 图集服务 GAP-T02。freeze 在 `otty-ui` 切 tab 时调 render 的 release/recreate。
- **完成:** 闲置 tab 在 `freeze-inactive-tab=true` 时不占 GPU 表面；切回第一帧能画出来。

---

# GAP-C 配置、主题、快捷键

库层是重建里最接近原产品的一层（同一份 key 表、同一份主题字节）。缺口全在「接到壳」。

### GAP-C01 Settings 写回 toml

- **status:** partial
- **prio:** P0
- **官网:** FEATURES §6。不整文件重生。改值只补丁来源行。Reset 写成注释 `(reset to default)`。list 整表替换。alias（`shell`）写入时改成真名 `command`。`config.local.toml` 覆盖 shared。
- **DMG:** `otty_config` 247 符号，`toml_edit` 167。Settings HTML（GAP-K03）只发「改这个 key」，Rust 改文件。
- **原实现:** UI 从不 `serde` 一轮再 dump。`toml_edit` 保注释与顺序。这是原产品明确写过的契约。
- **当前:** `otty-config` 能 load/set、119 测。CLI `otty config set/get --show-origin` 在。无 UI。写入路径要核对是否真用 toml_edit 补丁而不是重写。
- **落点:** 所有 UI（Settings、主题选择、keymap）只调 `otty-config`。发现重写整文件就改库，不要在壳里修。
- **完成:** 改一个 key 前后 diff 只有那一行（外加必要的空白）；文件头注释还在。

### GAP-C02 主题选择器

- **status:** partial
- **prio:** P1
- **官网:** FEATURES §6。25 个 bundled `.ottytheme`。Theme… 选当前。导入 iTerm2 / kitty / Alacritty / Ghostty；导出后三者。`customization/import-export` 页被爬虫截断。
- **DMG:** 25 份主题已抽到 `resources/themes/`。UI 字体那一行有意改成 Styrene（`docs/TYPOGRAPHY.md`）。
- **原实现:** 主题是独立文件，不是 config.toml 里一大段。当前主题名是一个 key。导入生成 `~/.config/otty/themes/<slug>.ottytheme`。
- **当前:** `otty-config` 读主题、import/export 库在。CLI 能列。无 Theme 菜单，无热切换 UI。
- **落点:** 菜单/Palette 调已有 load。热切换只换 palette 再重绘，不要重启窗。
- **完成:** 能从 25 个 bundled 里切走再切回；画面颜色跟着变。

### GAP-C03 keymap 热加载到 UI

- **status:** partial
- **prio:** P0
- **官网:** FEATURES §6。`keybind = trigger=action`。用户行替换默认，不追加。152 action 都能绑。无绑定的也在 Palette。
- **DMG:** `en.json` → `ACTIONS`。Swift 壳按 keymap 拦截 NSEvent，命中则执行 action，否则进 PTY。
- **原实现:** 一份 map，三处消费：菜单快捷键显示、Palette chord 列、运行时分发。改 toml 后热加载。
- **当前:** `packages/config/src/keybind.rs` 解析 + defaults。Electron `app.js` 把 Ctrl+T/W/1–9 写死，不读 keymap。Slint 无分发。152 个 action 几乎都没有 handler。
- **落点:** 壳启动取 `Config::keymap()`。keydown 先查 map 再决定 action 或 PTY。每加一个 GAP-S 的 UI，就给对应 action id 接 handler，不要再写死 chord。
- **完成:** `keybind = ctrl+t=new_tab` 改成别的 chord 后，新 tab 跟新 chord 走，Ctrl+T 进终端。

---

# GAP-A Agent

Otty 不替你跑 agent。它插进你已经在跑的 CLI：徽章、通知、resume、fork、历史。适配表、钩子脚本、`install_plan` 值已经在库里。缺口是对话框、画徽章、读 transcript、接 IPC。

钩子原文：`resources/agent-integration/`（从 DMG 拷来，11 份）。adapter 表：`packages/agent/src/lib.rs`。

### GAP-A01 一键装钩子

- **status:** partial
- **prio:** P1
- **官网:** FEATURES §4。第一次需要集成时问一句，只往该 agent 自己的配置里加 Otty 条目，其余不动。关掉就干净删掉。装完要重启该 agent 才接通。
- **DMG:** 各 agent 一份钩子。`Adapter::install_plan` 描述改哪个文件、加哪些键、怎么撤回。Grok 借用 `~/.claude/settings.json`，钩子里自检调用者，不是自己就站开。
- **原实现:** Swift 弹确认框，同意后 Rust 合并 JSON/TOML/插件目录。绝不整文件覆盖。OpenCode 是 plugin 文件拷进 `~/.config/opencode/plugins`。Pi/omp 是 extension 目录。
- **当前:** `install_plan` / `InstallKind` 有，23 测。无对话框、无写盘。
- **落点:** 计划已是值，可直接给对话框看。写盘用计划里的路径，测「已有其它 hooks 时只追加」。
- **完成:** 对 Claude 走一遍：确认 → `~/.claude/settings.json` 多出 Otty 段且原内容还在；再关一次，那段消失。

### GAP-A02 徽章与通知

- **status:** partial
- **prio:** P1
- **官网:** FEATURES §3 Progress、§4 Agent Behaviour。processing / idle / awaiting / error。不依赖 shell integration。开关：`privilege-badge-agent-*`、`privilege-notify-agent-*`。默认 processing 徽章文档有 conflict（Monitor 页 off，Configuration 页 on）。
- **DMG:** IPC `Request::AgentState`。钩子调 `otty state:<agent> state=… agent-pid=$$ session-id=…`。pane 匹配按进程树：pid 必须是某 pane shell 的后代。
- **原实现:** 钩子异步报状态。壳更新 tab 徽章、系统通知、Dock 转圈。awaiting 对手形。idle 对完成点。
- **当前:** `otty-agent` 有状态机和 `effects_for`。IPC 类型有。tab 上不画、不通知、钩子报上来没地方接。
- **落点:** 壳跑 IPC server（GAP-I01）后，把 `AgentState` 填进 `otty-ui` 的 pane.agent，再画 GAP-S10 的徽章。通知走 OS。
- **完成:** 跑一个装了钩子的 claude 回合，tab 出现 processing，idle 后变成完成点；Otty 不在前台时有系统通知。

### GAP-A03 历史 / Resume / Fork UI

- **status:** partial
- **prio:** P1
- **官网:** FEATURES §4。历史按路径识别 jsonl。Resume 用 adapter 的命令（Claude：`claude --resume <id>`）。Fork/Branch 是 GUI，不是 slash：四向 split / New Tab / New Window。Cursor/Kimi/CodeBuddy/Antigravity/Copilot 无 Fork。Grok/Reasonix 历史原产品也没接（GAP-U05）。
- **DMG:** `otty_agent_history` 50 符号、`otty_markdown` 11、`pulldown_cmark` 54。action `agent_fork*`、`agent_branch`。`otty-state` 的 `agent_session` 表记指针，不改 agent 自己的文件。
- **原实现:** 只读 agent 目录。阅读器当文件窗的一种 view（`view_as_agent_history` ↔ `view_as_jsonl`）。Resume：活着的 tab 就跳过去，否则新 pane 跑 resume 命令。Fork 跑 native fork 命令，新 pane 独立。
- **当前:** `resume_command` / `fork_command` / `supports_history` 有。无阅读器、无选落点 UI。
- **落点:** 扫描路径在 adapter；渲染用 markdown，不要自己造 jsonl 格式。落点选择器复用 GAP-S04 的 Split View picker。
- **完成:** Open Quickly → Agents 能列出本机一份 Claude jsonl；↩ resume；Fork in New Tab 开出第二条会话。

### GAP-A04 Usage / Quota

- **status:** partial
- **prio:** P2
- **官网:** FEATURES §4。Usage 只读 agent 自己写的 log。Quota 只 Claude / Codex / omp，后台跑 `claude -p "/usage"`、`codex app-server`、`omp usage --json`，每 5 分钟 + 任务结束再读。Codex 有封顶，条过 75% 黄、90% 红。费用只显示 agent 自己算的数。
- **DMG:** `quota_probe`、`bar_tone`、`UsageSupport` 在 `otty-agent`。
- **原实现:** 探针在后台，失败几次就停。Claude 探针用最便宜模型、关 session 持久，避免污染历史。画在 Details → Info 的 agent 组。
- **当前:** 函数有。不轮询、Details 没有 Info 卡（GAP-S06）。
- **落点:** 轮询放壳或小任务；数进 Info。不要对公开价卡去估 Claude 费用。
- **完成:** Codex 会话的 Info 里能看到一条带封顶的条；CLI 不在 PATH 时安静失败，不弹窗刷屏。

### GAP-A05 Skills 拷贝

- **status:** open
- **prio:** P2
- **官网:** FEATURES §4。Settings → Agents → Skills。`/wait` `/tell` `/open` `/otty`。Copy 一行，贴进 agent，agent 自己写 skill 文件。`/tell` 受 `ipc-allow-send-keys` 管。Skills 专页没抓到（GAP-K01）。
- **DMG:** 无独立 skill 文件。文案在 en.json。编排靠 CLI（GAP-I02）。
- **原实现:** 只提供可拷的 markdown。不替用户写进 `~/.claude`。
- **当前:** 无 Copy UI。CLI 表面有，执行不完整。
- **落点:** 先把 FEATURES 里 `/otty` 那段做成一份 markdown 资源。Settings 一键拷到剪贴板。真编排等 GAP-I02。
- **完成:** Settings 能拷出一份含 `otty pane split/run/wait` 的 skill 文本。

### GAP-A06 防休眠 helper

- **status:** open
- **prio:** P2
- **官网:** FEATURES §4。Prevent Sleep While Processing，按 tab。合盖仍睡。Also When the Lid Is Closed 要 root 写系统项，先装后台 helper，用户在登录项里批准。Otty 必须在 Applications 里。helper 跟活连接走，崩了就还睡眠。
- **DMG:** `Contents/MacOS/otty-sleep-helper`（arm64 123 584 / x64 85 968）+ `Library/LaunchDaemons/io.appmakes.otty.sleep-helper.plist`。
- **原实现:** 独立二进制，不是 Swift 里调 `caffeinate`。断言绑在「有 processing 且开关开」的 tab。
- **当前:** 无 helper、无 plist。徽章枚举里有咖啡，没人设置。
- **落点:** macOS only。不要在 Windows 上仿。重建用独立 bin + launchd，不要塞进主进程。
- **完成:** 开关开、agent processing 时 `pmset -g assertions` 能看到 Otty；idle 后消失。

---

# GAP-W 工作流

### GAP-W01 Recipes

- **status:** open
- **prio:** P1
- **官网:** FEATURES §5。范围 tab / window / commands。内容 Layout Only / Include Scrollback / Include Commands。内部库 `~/.config/otty/recipes/`。导出 `.ottyrecipe` 不含 scrollback。信任按 SHA-256，记在 `trusted_recipes.json`。回放：auto / ask_once / manually（页上还有 Skip，和 Configuration 三值 conflict）。ssh/tmux/docker exec 会暂停后续。
- **DMG:** action `save_recipe`、`save_recipe_as`。snippets 走 autocomplete 别名。
- **原实现:** 内部库 SQLite 或目录 + toml。打开 `.ottyrecipe` 先过信任框。路径可变成 `{{current_folder}}` / `{{home}}` / `{{recipe_location}}`。
- **当前:** 无解析器、无 UI、无信任表。
- **落点:** 格式按 FEATURES 那段 TOML。回放命令走 GAP-I02。安全框不要省。
- **完成:** 存一个两 pane 的 window recipe，再打开能长出同样的树和 cwd；未信任文件会问。

### GAP-W02 启动恢复

- **status:** partial
- **prio:** P0
- **官网:** FEATURES §5。干净退出跟 `on-launch`（默认 `restore_session`）。崩溃无条件恢复，连崩几次改开空窗。更新后总是恢复。关 tab 栈 12 条（⇧⌘T）。Open Recent 10 条。`session-restore-multiplayer` 重贴 tmux。`privilege-resume-agent-session` 恢复 agent。`session-restore-processes` none/whitelist/all。banner 默认 on。
- **DMG:** `otty_state` session 表。原产品每次 tab 创建/关闭/改布局写快照。
- **原实现:** 启动读库，按窗重建 tab 树、cwd、scrollback（session log）。agent 走 adapter resume。命令重跑看白名单，前缀匹配。
- **当前:** `otty-state` schema v3 有 session 表。启动不读。Electron 关 tab 栈只在内存。无 banner、无进程重跑。
- **落点:** 快照写在 `otty-ui` 改树时。启动在 `otty-shell`。agent 恢复复用 GAP-A03。
- **完成:** 开两个 tab、关 Otty、再开，两个 cwd 都在；⇧⌘T 能把刚关的那个要回来。

### GAP-W03 Frecency UI

- **status:** partial
- **prio:** P1
- **官网:** FEATURES §5。内置 zoxide。自动 OSC 7、`otty learn`、`otty import zoxide`。顶 100。衰减：1h×4、1d×2、1w×0.5、更旧×0.25。Manage Folders 编辑器。`otty ignore` 进黑名单且粘。
- **DMG:** `frecency` 表。CLI `jump`/`learn`/`ignore`。
- **原实现:** 分数在库里算。Open Quickly Folders 与 `otty jump` 共用。无 shell 插件。
- **当前:** 库 + CLI 在。无 Manage 窗。OSC 7 更新 cwd，未必写 frecency。
- **落点:** OSC 7 处理处插 `frecency-auto-record`。UI 可挂 Settings 或 Open Quickly。
- **完成:** 在 pane 里 cd 几次后 `otty jump <那目录的片段>` 能跳到；Ignore 之后不再出现。

### GAP-W04 SSH 远程 pane

- **status:** partial
- **prio:** P2
- **官网:** FEATURES §5 写 Remote development 仍 In Development。已交付：ssh 包装、terminfo 探测（`otty ssh terminfo` / `doctor`）、转发 shell integration、Devices 徽章、Open Quickly SSH filter。
- **DMG:** `otty_ssh_config` 23 符号。`otty-helper` 四份（darwin/linux × arm64/x64 musl），上传到远端。`HostStreamTransport` 3 个 impl 符号：session 可以是「这边的 pty」或「那边的流」。
- **原实现:** 本地 `ssh` 仍是普通 PTY。helper 提供远端 stat/传文件。真正的远程 pane 传输层在 `HostStreamTransport`，文档没写完。
- **当前:** `otty-ssh` 解析 config、helper 协议有（24 测）。只有本地 PTY。无 Devices 徽章。
- **落点:** 先接 Open Quickly SSH（↩ 在当前 pane 跑 `ssh host`）和 Devices 徽章。`HostStreamTransport` 等 U03/U06 不再 skip 再做。
- **完成:** Open Quickly → SSH 能列出 `~/.ssh/config` 里一个 Host，↩ 连上；远端会话 tab 有设备徽章。

---

# GAP-I CLI 与 IPC

规则：无 GUI 的子命令走本地；驱动窗的走 IPC。退出码是接口：3 没在跑、4 无此 pane、6 无法报告 idle、9 超时、13 闸门拒绝。

### GAP-I01 壳内 IPC server 全套

- **status:** partial
- **prio:** P0
- **官网:** FEATURES §5–6。一人一个 socket。POSIX Unix socket，Windows named pipe。闸门：`ipc-allow-send-keys`（默认关）、`ipc-allow-sensitive-sessions`（默认关，挡 ssh/sudo）。
- **DMG:** 原产品壳进程听 socket。CLI 短连接、一行 JSON 去一行 JSON 回。
- **原实现:** 请求类型见 `packages/ipc/src/lib.rs`：Window/Tab/Pane/AgentState/ReloadConfig/Badge/Notify/SetTheme/Status。敏感 pane 在 server 侧拒，不在 CLI 侧。
- **当前:** 类型 + 17 测。Unix 路径是实现。壳没听。Windows named pipe 只写了文档。
- **落点:** `otty-shell` 启动时 bind。每个 `Request` 调 `otty-ui`。Electron 若还当入口，也要听同一 socket，否则 `otty tab new` 找不到 app。
- **完成:** 壳开着时 `otty window list` 退出 0 并打印一窗；关掉后退出 3。

### GAP-I02 pane run / exec / capture / wait

- **status:** open
- **prio:** P0
- **官网:** FEATURES §5。`send-text` 解释 `\x03`，不自动加换行。`run` 提交后等到结束，返回进程状态不是输出。`exec` 拆 stdout/stderr。`wait` 什么都不打，idle 即 0；pane 两边都不能报则 6。`capture` 是屏幕网格快照。agent pane 的 idle 以钩子为准，不是 OSC 133。
- **DMG:** `PaneRequest` 已列全。AppleScript `contents`/`history`/`busy` 走同一套。
- **原实现:** run 需要 shell integration + 空闲 prompt。闸门关时 send-keys 退 13。
- **当前:** clap 有。没有执行。
- **落点:** 接在 GAP-I01 后面。wait 对 agent 看 GAP-A02，对 shell 看 OSC 133 D。capture 从 `otty-term` grid 取。
- **完成:** `otty pane send-text --pane $OTTY_PANE_ID 'echo hi\n'` 在该 pane 出现；`wait` 在命令结束后退 0。

### GAP-I03 `otty://` 与 Install CLI

- **status:** open
- **prio:** P1
- **官网:** FEATURES §5。scheme：`otty://window|tab|pane/<sel>`，可带 `?line=`。Install CLI：symlink `/usr/local/bin/otty`、装 bash/zsh/fish 补全、给已装的 vim/bat 装语法。Omit prefix 给 edit/view/watch/jump/learn 装壳函数。
- **DMG:** Info.plist 3 个 URL scheme、5 个文档类型。`resources/editors/` 已拷。shell-integration 里的 wrapper 已拷。
- **原实现:** macOS 用 Launch Services 把 `otty://` 交给主程序。Windows 对等协议。CLI 安装要管理员时再要一次。
- **当前:** plist 没组进包。scheme 无 handler。补全子命令 `otty completions` 有 clap。
- **落点:** handler 把 URL 收成 IPC。Install 是 Settings 一行，调已有 completions + editors 资源。
- **完成:** `otty pane link` 打出一条 URL；打开它会聚焦那个 pane。Settings 能把 `otty` 放到 PATH。

---

# GAP-P 打包与平台

### GAP-P01 macOS `.app`

- **status:** open
- **prio:** P3
- **官网:** macOS 14+。bundle id `io.appmakes.otty`。版本 1.5.4。
- **DMG:** 完整 `.app`：`MacOS/Otty`、`otty-cli`、`rg`、`otty-sleep-helper`、Frameworks、Resources、Info.plist 88 key（`LSMinimumSystemVersion=14.0`、`NSAppleScriptEnabled`、`OttyGitHash`）。签名 xa 在 7-Zip 抽取时丢了。
- **原实现:** Swift 主程序链入 Rust staticlib。重建没有 Swift，主程序是 `otty-app`。
- **当前:** 无组包产物。`docs/BUILDING.md` 写了目标布局。
- **落点:** 按 BUILDING 的 Contents 表装配。`OTTY_GIT_HASH` 注入。公证另做。
- **完成:** 一台 macOS 上双击 `.app` 能开窗；`Otty.app/Contents/MacOS/otty-cli --version` 有哈希。

### GAP-P02 helper 四 triple

- **status:** partial
- **prio:** P2
- **官网:** helper 上传到 ssh 对面，所以 macOS 包里带 Linux musl。
- **DMG:** `Resources/helper/{aarch64,x86_64}-apple-darwin` 与 `{aarch64,x86_64}-unknown-linux-musl`。两份 macOS 包里的 helper 树字节相同。
- **原实现:** 同一源码交叉编四次。Linux 用 musl 为了静态、不挑发行版。
- **当前:** `packages/helper` 1250 行 / 24 测。本机只有 `x86_64-pc-windows-msvc`，没交叉。
- **落点:** `docs/BUILDING.md` 的 `rustup target add` + 交叉。产物放 `resources/helper/`。
- **完成:** 四个 triple 各有一个能跑的 `otty-helper`；`otty ssh doctor` 能说到 helper。

### GAP-P03 捆绑 rg

- **status:** open
- **prio:** P2
- **官网:** Search in Files 用内置 ripgrep，不走 PATH。
- **DMG:** `Contents/MacOS/rg`（arm64 4 010 304 / x64 4 440 480）。
- **原实现:** 第三方二进制，不是 Otty 源码。
- **当前:** 未打包。GAP-S12 依赖它。
- **落点:** 组包时放进 MacOS/（或 resources），代码用捆绑路径。不要要求用户装 rg。
- **完成:** 没有系统 rg 时，Search in Files 仍能搜。

### GAP-P04 AppleScript

- **status:** open
- **prio:** P2
- **官网:** FEATURES §7。1.2.0+。对 Terminal.app 常用动词 drop-in。`do script` 返回 tab。`contents` / `history` / `busy` / `processes` / `tty` / rows/cols / `working directory` / `id`。ssh/sudo 上拒绝，和 CLI 闸门同一套。
- **DMG:** `resources/Otty.sdef` 已拷（5580 B）。Info.plist `NSAppleScriptEnabled`、`OSAScriptingDefinition`。sdef 注释写实现在 `Sources/Packages/AppleScript/`（Swift，源码没了）。Cocoa 类名：`OttyDoScriptCommand`、`OttyScriptingTab`。
- **原实现:** sdef 映射到和 IPC 相同的 pane 原语。重建应走 `otty-ipc`/`otty-ui`，不要再做一套。
- **当前:** 字典在，没注册、没命令类。
- **落点:** macOS 壳用 sdef。每个属性转成已有 PaneRequest。
- **完成:** `osascript -e 'tell application "Otty" to do script "echo hi"'` 开一个 tab 并跑命令。

### GAP-P05 VoiceOver

- **status:** open
- **prio:** P2
- **官网:** FEATURES §7。自定义渲染，但暴露 viewport 文本、选区、光标、tab/pane 铬、常用命令。只读可见 viewport，不一次读全部 scrollback。图无 alt。TUI 内部按钮不是原生控件。
- **DMG:** `NSAccessibility`。Swift 壳。
- **原实现:** 终端当一块文本，带行/字符范围。光标和选区变化时发更新，避免每帧轮询。
- **当前:** 无。
- **落点:** 只在 macOS 壳做。数据从当前 grid 的可见行来。
- **完成:** VoiceOver 能读当前屏幕一行，并跟着光标走。

### GAP-P06 Dock / Secure Input / 托盘

- **status:** open
- **prio:** P2
- **官网:** Dock：`dock-icon-animate-progress` 默认关，`dock-icon-error-badge` 默认开，点 Dock 跳到下一个失败 tab。Secure Input：`auto-secure-input` 在 canonical-no-echo（sudo/ssh/login）时开，完了关；标题栏药丸。Windows 托盘：`stay-in-tray` / `show-tray-icon`。
- **DMG:** Dock 是 AppKit。Secure Keyboard Entry 是 macOS API。托盘是 Win32。
- **原实现:** Dock 聚合所有 tab 的 OSC 9;4。Secure Input 看前台进程 tty 模式，不是猜密码字符串。
- **当前:** 无。
- **落点:** 三件事先做 Secure Input（安全），再 Dock，托盘最后。
- **完成:** sudo 密码提示时 Secure Input 药丸出现；命令结束后消失。

---

# GAP-U 原产品也没有（默认不排期）

这些不是重建欠债。总表留着，免得以后又当缺口开票。只有明确「要对齐二进制里的隐藏面」才改 status。

### GAP-U01 Status bar

- **status:** skip
- **官网:** FEATURES §1。「Planned, not implemented yet」，页上没别的。
- **DMG:** 无对应 UI 资源。
- **当前:** Electron 里有一条 statusbar，CSS 里 `display: none`，注释写 Otty 还没有。
- **完成:** 不要做，除非官网改口。

### GAP-U02 Data Sync

- **status:** skip
- **官网:** FEATURES §5 In Development。现在就靠同步 `~/.config/otty`。
- **DMG:** `otty_sync_dotfiles` 187 符号、`otty_sync_transport` 45、`tokio`/`hyper`/`reqwest`/`rustls`/`ring`/`ed25519_dalek`/`argon2`。二进制里有加密传输和配额类型。
- **原实现:** 没对外完工。重建故意不拉网络栈。
- **完成:** 默认不做。

### GAP-U03 Remote development

- **status:** skip
- **官网:** FEATURES §5 In Development。普通 ssh 能用。
- **DMG:** 见 GAP-W04。`HostStreamTransport` 已链进包。
- **完成:** 先把 W04 的包装/徽章做完。真正的远程 pane 等产品要。

### GAP-U04 Sixel

- **status:** skip
- **官网:** vt-sequences ☐ / planned。DCS 页写不支持。Kitty 图形覆盖同一需求。
- **DMG:** `SixelScanner` 1 个 Default impl 符号，无解码器。
- **当前:** 与原产品一致，不解析。
- **完成:** 不要做。

### GAP-U05 Grok / Reasonix 历史等

- **status:** skip
- **官网:** FEATURES §4。Grok、Reasonix：history 没接线。CodeBuddy / Antigravity / Copilot：无 history、usage、Fork。
- **DMG:** 钩子文件在，adapter 表有 resume 命令。
- **完成:** 只做徽章/resume 命令。不要假装有历史阅读器。

### GAP-U06 Otty Anywhere / 远程控制面

- **status:** skip
- **官网:** 几乎不写。和 Data Sync 一起算 In Development。
- **DMG:** `otty_anywhere` **107 符号**（主程序 crate 里排第五）。消息：`ControlIn` / `ControlOut` / `SizeClaim` / `PaneState` / `Permission`。另有 `otty_remote_shim` 8 符号。这是包里的隐藏控制面，不是文档功能。
- **原实现:** 重建没做任何出站网络。
- **完成:** 默认不做。若要对齐二进制能力，这是单独项目，不是随手一条 GAP。

---

卡片到此写完。总表里每一行都有对应卡片。建议开工顺序：K02/K03 资源 → I01 IPC → C03 keymap → S07 分屏 → W02 恢复 → S04 Palette → S06 Details → A01/A02 钩子与徽章。U 组不要排进迭代。
