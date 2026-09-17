# 供应商、Key 与云备份实施

> 状态：待审，未开始写代码  
> 日期：2026-09-17  
> 对应规格：[项目描述.md](./项目描述.md) v1.3  
> 实现顺序：本文阶段顺序优先于 [springcat-ai-v1.md](./springcat-ai-v1.md) 的宠物模式（S10+）  
> 对照产品：[CC Switch](https://github.com/farion1231/cc-switch)，同类能力在 SpringCat 里原生实现，不嵌外部应用

本文给审查用。通过后再拆进 `springcat-ai-v1.md` 的具体步骤。没勾选的阶段不要提前做。

---

## 1. 要做成什么

SpringCat 继续是一个桌面入口。用户在这里：

1. 给 Codex / Gemini CLI / Grok CLI 保存多套供应商（名称、Base URL、API Key）
2. 一键把当前套写回对应工具的本机配置，不用手改 JSON / TOML / `.env`
3. 以后可选地把 SpringCat 自己的数据备份到云端（换机、防丢失）

任务监听、用量、跳转保持原样。供应商管理和监听是两条线，写 Key 时不得冲掉 SpringCat 已经装上的 hooks。

## 2. 和 CC Switch 的关系

| CC Switch | SpringCat 怎么做 | 阶段 |
|---|---|---|
| 多套 Provider、一键启用 | 原生供应商库 + 写回本机配置 | A–C |
| 托盘快切 | 现有托盘增加子菜单 | D |
| 50+ 中转预设 | 少量内置预设，可后补 | E |
| 一份配置同步到多个工具 | 可选「通用供应商」 | E |
| 云同步（文件夹 / WebDAV） | 可选备份，默认关闭，Key 加密 | F |
| 统一 MCP / Skills / Prompts | 不做，除非单独再开阶段 | 候补 |
| 本地反向代理、故障转移 | 明确不做 | 不做 |
| 嵌进 SpringCat 当子应用 | 明确不做 | 不做 |

第一期覆盖 **Codex、Gemini CLI、Grok CLI**。Cursor、WorkBuddy、Marvis、ZCode、DSH 第一期不改 Key。

## 3. 全程约束

- 默认本地优先。没有云账号时，改 Key、切换必须完整可用。
- API Key 只进本机供应商库，切换时写入目标工具配置。不进任务事件、inbox、用量库、tracing 正文、UI 日志。
- 界面默认遮罩 Key，用户点「显示」才看见。
- 写入目标配置必须：先备份 → 临时文件 → rename。失败回滚。SpringCat hooks 必须还在。
- 不新开 HTTP / WebSocket 端口，不为改 Key 做本地代理。用户用中转，就把中转的 Base URL 和 Key 存成一套供应商。
- 不复制各工具聊天界面，不读取项目源代码，不上传完整对话。
- 现有 `adapter_installer.rs` 的合并写入、`.springcat.bak`、原子 rename 是写入层的参考，供应商写入走独立模块，不要把 Key 写进 hook 安装逻辑。

建议模块：

```text
src-tauri/src/providers/
  mod.rs              # 对外 command
  store.rs            # SQLite 供应商库
  backup.rs           # 切换前备份 / 回滚
  writers/
    codex.rs
    gemini.rs
    grok.rs
src/components/settings/providers/
```

供应商库与任务库可以同属一个 SQLite 文件、分表；Key 列不得出现在任务查询里。

---

## 阶段 A · 本机供应商库

**目标：** 能增删改查供应商，还不能写 CLI 配置。

**用户能做的：** 无界面也可，先以后端和测试为准。本阶段结束时设置页可以先空着。

**要做：**

- 表 `providers`：`id`、`app`（`codex` / `gemini-cli` / `grok-cli`）、`name`、`base_url`、`api_key`、`sort_order`、`created_at`、`updated_at`、`notes`
- 表 `provider_state`：每个 `app` 一行，记录 `active_provider_id`
- Tauri command：`list_providers`、`upsert_provider`、`delete_provider`（不允许删当前启用项，或删前必须先切走）
- 列表接口返回遮罩后的 Key（例如只留后 4 位）；完整 Key 只在「显示」或「写入」时走单独 command
- 单测：CRUD、不能把 Key 写进任务表、删除当前启用项被拒绝

**验收：**

- 只用 Rust 测试即可证明库存活
- `tracing` 里看不到完整 Key
- 浏览器演示不需要连真实 CLI

**本阶段不做：** 写 `~/.codex`、设置页、托盘、云。

---

## 阶段 B · Codex 切换可用

**目标：** 在 SpringCat 里启用某一套 Codex 供应商后，Codex 下一次请求走这套 Base URL 和 Key。

**用户能做的：**

1. 打开设置 → 新标签「供应商」→ Codex
2. 新增一套：名称、Base URL、API Key
3. 点「启用」后，本机 Codex 配置被更新
4. 切回「官方登录」预设时，恢复官方用法（OAuth / 官方 auth），不是只清空 Key

**要做：**

- `writers/codex.rs`：在实现时对照当时 Codex 文档，确认写入哪些文件（预期是 `~/.codex/config.toml` 和/或 `~/.codex/auth.json`）。不要凭记忆锁死字段名。
- 启用前把将要改的文件拷到 `~/.codex/springcat-provider-bak/`（或现有 `.springcat.bak` 策略的升级版），保留最近若干份。
- 写入后校验：目标文件可解析，SpringCat 的 `hooks.json` 仍在且命令未变。
- 设置页 Codex 列表：启用中标记、遮罩 Key、失败时显示原因和备份路径。
- 首次进入 Codex 页：把当前本机配置导入为「当前配置」供应商，避免用户一启用就把原来的官方登录冲掉。

**验收：**

- 准备两套测试供应商（可用假 Key + 可解析配置）。启用 A 后文件内容是 A；再启用 B 后是 B；再切回导入的「当前配置」，Codex 官方登录相关字段恢复。
- 故意写坏 JSON/TOML：界面报错，原文件保持启用前内容。
- `~/.codex/hooks.json` 里的 SpringCat hook 一条不丢。
- 不要求本阶段改终端热重载；文档里写明：Codex 是否要重开终端，以当时 Codex 行为为准。

**本阶段不做：** Gemini、Grok、托盘、预设大全、云。

---

## 阶段 C · Gemini CLI 与 Grok CLI

**目标：** 和 Codex 同一套供应商库、同一设置页，换不同 writer。

**用户能做的：** 在「供应商」里切换 Gemini CLI、Grok CLI 的当前套。三个工具的启用项互相独立，切 Codex 不会改 Gemini。

**要做：**

- `writers/gemini.rs`：实现时核对 `~/.gemini/.env`、`~/.gemini/settings.json` 里 Key / Base URL 的现行字段。只改供应商相关字段，保留 SpringCat 已经合并的 `hooks`。
- `writers/grok.rs`：实现时核对 Grok CLI 现行配置位置（家目录下的 config / env，不要假设和 hooks 是同一个文件）。
- 每个 app 独立 `active_provider_id`。
- 设置页按工具分栏或分列表，复用 Codex 页组件。
- 各自的导入「当前配置」、备份、回滚、hook 保护，规则与阶段 B 相同。

**验收：**

- 切 Gemini 不影响 `~/.codex` 和 `~/.grok`。
- 切 Grok 不影响 Gemini hooks 与 Codex hooks。
- Gemini `settings.json` 里 SpringCat `hooks` 仍在。
- Grok 的 `~/.grok/hooks/springcat.json` 仍在。
- 三个 writer 都有失败回滚测试。

**本阶段不做：** 一份 Key 同时写三个工具、托盘、云。

---

## 阶段 D · 托盘快切与导入打磨

**目标：** 不必打开设置也能换当前供应商；误操作成本低。

**用户能做的：**

- 托盘出现「Codex / Gemini / Grok」子菜单，列出已保存供应商，当前项打勾
- 点名称即启用，短暂提示成功或失败（沿用现有低干扰策略，不要新弹大窗）
- 设置页可把正在用的本机配置重新导入、改名、排序

**要做：**

- 扩展 `src-tauri/src/tray.rs`，子菜单随供应商列表刷新
- 启用失败时托盘提示失败原因，不静默
- 空列表时子菜单显示「还没有供应商」，点开设置对应标签
- 删除保护：当前启用项不能删；最后一个官方/导入项不能删到让 CLI 完全没配置（与 CC Switch「最少侵入、卸掉软件 CLI 还能跑」一致）

**验收：**

- 托盘切 Codex 供应商，设置页的启用标记一起变
- 设置窗口没开时也能切
- 三个工具子菜单互不串

**本阶段不做：** 云、MCP、代理。

---

## 阶段 E · 预设与通用供应商（可选）

**目标：** 降低第一次上手成本。本阶段可在 D 之后做，也可以审稿时砍掉或推迟。

**用户能做的：**

- 新增时从少量内置预设选（官方、自定义、以及 3–5 个常见 OpenAI 兼容中转模板）。只填 Key 和可选 Base URL。
- 「通用供应商」：同一套名称 / Key / Base URL，勾选要写入的 app（Codex / Gemini / Grok），启用时按各 writer 各自写一份。

**要做：**

- 预设放在仓库静态数据里，不从远程拉（避免启动依赖网络，也避免变成推广墙）
- 不内置任何第三方中转的推广文案或邀请码
- 通用供应商在库里是一条记录 + 多个 app 绑定，不是复制三份互不同步的副本；启用时仍走各 app writer

**验收：**

- 选「自定义」可以只填 Base URL + Key
- 通用供应商启用后，被勾选的工具配置都更新，未勾选的不变
- 预设不改用户已保存的自定义供应商

**本阶段不做：** 50+ 商业中转目录、赞助商入口。

---

## 阶段 F · 可选云端备份

**目标：** 换机或重装后能恢复设置、供应商（含 Key）、任务元数据、用量汇总。默认关闭。

**前置：** 阶段 C 可用。没有云账号时 A–D 不受影响。

**用户能做的：**

1. 设置里打开「云端备份」
2. 选择方式：指定同步文件夹（iCloud / OneDrive / Dropbox / NAS 盘符）或 WebDAV
3. 手动「立即备份」和「从备份恢复」
4. 可随时关闭，关闭后不再上传

**要做：**

- 备份包内容：设置、供应商表、任务生命周期元数据、用量汇总。不含 inbox 原文、完整对话、项目代码。
- Key 必须加密后再进备份包。口令由用户设；没有口令不允许上传 Key。恢复时要口令。
- 默认不同步；第一次打开要明确告诉用户 Key 会进加密备份。
- 不做 SpringCat 账号系统，不做社交，不做自动多端冲突合并的第一版。冲突策略第一版：恢复前让用户选「用云端覆盖本机」或取消。
- 自动定时备份可以后补；第一版手动备份即可。

**验收：**

- 默认关闭时，数据目录之外看不到供应商备份文件
- 用错误口令无法读出 Key
- 恢复后三个工具的供应商列表和启用项与备份时一致；然后用户仍需在本机点一次「启用」才会写回 CLI 配置（避免恢复过程悄悄改正在跑的 CLI）
- 备份包里没有 prompt / 工具结果

**本阶段不做：** 自建服务器、登录体系、实时多端同步、备份完整会话 transcript。

---

## 候补（本文不排期）

审稿时若要做，另开文档：

- 统一 MCP / Skills / `AGENTS.md` 同步
- 更多工具的 Key 切换（Claude Code、OpenCode 等）
- 供应商纬度的用量拆分
- 本地反向代理与故障转移（与「不开放本地 HTTP 端口」冲突，默认拒绝）

---

## 建议发布切片

| 可对用户发布 | 包含阶段 | 用户价值 |
|---|---|---|
| 第一刀 | A + B | Codex 能在 SpringCat 里改 Key |
| 第二刀 | C + D | 三个已监听 CLI 都能切，托盘能切 |
| 第三刀 | E | 上手更快 |
| 第四刀 | F | 换机不丢配置 |

宠物模式仍后置，不挡上述切片。

---

## 请审的问题

1. 设置里「供应商」是独立标签，还是放进现有「AI 工具」页？本文默认独立标签，避免和 hook 绑定混在一起。
2. Codex「官方登录」是必须做的第一期恢复路径，还是第一期只做 API Key / Base URL，官方 OAuth 放后？
3. 阶段 E 的内置中转预设要不要做？默认建议少做或不做商业预设，只留官方 + 自定义。
4. 本机 Key 是否进系统钥匙串（Windows Credential Manager / macOS Keychain）？第一期也可以明文进 SQLite，只保证不进日志；钥匙串更安全但实现更重。
5. 阶段 F 的第一版只做「指定文件夹」，WebDAV 是否一起做？
6. 恢复云备份后，是否同意「不自动写回 CLI，用户再点一次启用」？这样更安全。

审完这 6 点并确认阶段范围后，再把通过的阶段拆进 `springcat-ai-v1.md` 开写。
