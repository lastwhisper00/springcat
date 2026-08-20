<script lang="ts">
  import { isTauri } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import {
    DEFAULT_SETTINGS,
    type ClientSettings,
    type DockSide,
    type DoubleClickAction,
  } from "$domain/settings";
  import type { UsageSource } from "$domain/usage";
  import {
    adapterBindInfo,
    adapterInstallStatus,
    emitAdapterTest,
    getBrowserInfo,
    getSettings,
    getStorageInfo,
    installAdapter,
    quitApp,
    restartApp,
    uninstallAdapter,
    updateSettings,
    type AdapterBindInfo,
    type AdapterInstallStatus,
    type AdapterSource,
    type BrowserInfo,
    type StorageInfo,
  } from "$services/tauri";
  import ToolLogo from "$components/work-panel/ToolLogo.svelte";
  import brandLogo from "../../assets/branding/透明.png";
  import { ADAPTERS, bindPrompt } from "./adapter-prompts";
  import Button from "./controls/Button.svelte";
  import SelectField from "./controls/SelectField.svelte";
  import SettingsCard from "./controls/SettingsCard.svelte";
  import Toggle from "./controls/Toggle.svelte";
  import ToggleRow from "./controls/ToggleRow.svelte";
  import UsageCalendar from "./UsageCalendar.svelte";

  type Tab = "general" | "adapters" | "usage";
  type InstallStatusMap = Partial<Record<AdapterSource, AdapterInstallStatus>>;

  const browserPreview = !isTauri();
  let settings = $state<ClientSettings | null>(null);
  let saving = $state(false);
  let tab = $state<Tab>("general");
  let expandedSource = $state<AdapterSource | null>(null);
  let bind = $state<AdapterBindInfo | null>(null);
  let copiedSource = $state<AdapterSource | null>(null);
  let testing = $state(false);
  let installing = $state(false);
  let removing = $state(false);
  let installStatuses = $state<InstallStatusMap>({});
  let testNotes = $state<Partial<Record<AdapterSource, string>>>({});
  let storageInfo = $state<StorageInfo | null>(null);
  let storageNote = $state("");
  let choosingStorage = $state(false);
  let browserInfo = $state<BrowserInfo | null>(null);
  let choosingBrowser = $state(false);

  const storageDirectory = $derived(
    storageInfo?.configuredDirectory ?? storageInfo?.defaultDirectory ?? settings?.cacheDirectory ?? "",
  );
  const boundUsageSources = $derived(
    settings
      ? ([
          ...(settings.adapters.codex && sourceInstalled("codex") ? ["codex"] : []),
          ...(settings.adapters.cursor && sourceInstalled("cursor") ? ["cursor"] : []),
          ...(settings.adapters.grokCli && sourceInstalled("grok-cli") ? ["grok-cli"] : []),
          ...(settings.adapters.marvis && sourceInstalled("marvis") ? ["marvis"] : []),
        ] as UsageSource[])
      : [],
  );

  type SelectOption = { value: string; label: string; disabled?: boolean };

  const browserOptions = $derived<SelectOption[]>([
    { value: "", label: `跟随系统（${browserInfo?.systemDefaultName ?? "检测中…"}）` },
    ...(browserInfo?.browsers ?? []).map((browser) => ({
      value: browser.path,
      label: browser.name,
    })),
    ...(settings?.browserPath &&
    browserInfo &&
    !browserInfo.browsers.some((browser) => browser.path === settings?.browserPath)
      ? [{ value: settings.browserPath, label: browserNameFromPath(settings.browserPath) }]
      : []),
  ]);

  async function patch(partial: Partial<ClientSettings>) {
    if (browserPreview && settings) {
      settings = {
        ...settings,
        ...partial,
        adapters: partial.adapters ? { ...settings.adapters, ...partial.adapters } : settings.adapters,
      };
      return;
    }
    saving = true;
    try {
      settings = await updateSettings(partial);
    } finally {
      saving = false;
    }
  }

  function setTestNote(selected: AdapterSource, note: string) {
    testNotes = { ...testNotes, [selected]: note };
  }

  function isPassiveSource(selected: AdapterSource): boolean {
    return selected === "workbuddy" || selected === "marvis" || selected === "dsh-desktop";
  }

  async function copyPrompt(selected: AdapterSource) {
    try {
      const prompt = bind
        ? bindPrompt(selected, bind.bridgePath, bind.inboxDir)
        : "读取绑定信息…";
      await navigator.clipboard.writeText(prompt);
      copiedSource = selected;
      setTimeout(() => {
        if (copiedSource === selected) copiedSource = null;
      }, 1600);
    } catch {
      if (copiedSource === selected) copiedSource = null;
    }
  }

  async function runTest(selected: AdapterSource) {
    if (browserPreview) {
      setTestNote(selected, "预览环境：监听链路状态正常。");
      return;
    }
    testing = true;
    setTestNote(selected, "");
    try {
      const result = await emitAdapterTest(selected);
      setTestNote(selected, result.message);
    } catch (err) {
      setTestNote(selected, err instanceof Error ? err.message : "测试失败");
    } finally {
      testing = false;
    }
  }

  async function applyStorageDirectory(value: string) {
    if (browserPreview) {
      storageNote = "预览环境不会修改缓存目录。";
      return;
    }
    saving = true;
    storageNote = "";
    try {
      settings = await updateSettings({ cacheDirectory: value });
      storageInfo = await getStorageInfo();
      storageNote = storageInfo.restartRequired
        ? "已有任务历史已复制到新目录。重启 SpringCat 后开始使用新位置。"
        : "缓存目录已更新。";
    } catch (err) {
      storageNote = err instanceof Error ? err.message : "缓存目录设置失败";
    } finally {
      saving = false;
    }
  }

  async function chooseStorageDirectory() {
    if (browserPreview) {
      storageNote = "预览环境不会打开系统目录选择器。";
      return;
    }
    choosingStorage = true;
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        defaultPath: storageDirectory || undefined,
        title: "选择 SpringCat 缓存目录",
      });
      if (typeof selected === "string") {
        await applyStorageDirectory(selected);
      }
    } finally {
      choosingStorage = false;
    }
  }

  async function chooseBrowserExecutable() {
    if (browserPreview) return;
    choosingBrowser = true;
    try {
      const selected = await open({
        directory: false,
        multiple: false,
        title: "选择浏览器程序",
        filters: [{ name: "浏览器程序", extensions: ["exe"] }],
      });
      if (typeof selected === "string") {
        await patch({ browserPath: selected });
      }
    } finally {
      choosingBrowser = false;
    }
  }

  function browserNameFromPath(path: string): string {
    const executable = path.split(/[\\/]/).pop()?.toLowerCase();
    if (executable === "chrome.exe") return "Google Chrome";
    if (executable === "msedge.exe") return "Microsoft Edge";
    if (executable === "firefox.exe") return "Mozilla Firefox";
    if (executable === "brave.exe") return "Brave";
    if (executable === "vivaldi.exe") return "Vivaldi";
    return path.split(/[\\/]/).pop() ?? "自定义浏览器";
  }

  async function loadInstallStatus(selected: AdapterSource) {
    if (browserPreview) return;
    try {
      const next = await adapterInstallStatus(selected);
      installStatuses = { ...installStatuses, [selected]: next };
    } catch (err) {
      setTestNote(selected, err instanceof Error ? err.message : "读取安装状态失败");
    }
  }

  async function installSource(selected: AdapterSource) {
    if (browserPreview) {
      installStatuses = { ...installStatuses, [selected]: previewInstallStatus(selected, true) };
      if (settings) {
        const adapters = { ...settings.adapters };
        if (selected === "codex") adapters.codex = true;
        if (selected === "cursor") adapters.cursor = true;
        if (selected === "grok-cli") adapters.grokCli = true;
        if (selected === "gemini-cli") adapters.geminiCli = true;
        if (selected === "workbuddy") adapters.workBuddy = true;
        if (selected === "marvis") adapters.marvis = true;
        if (selected === "dsh-desktop") adapters.dshDesktop = true;
        settings = { ...settings, adapters };
      }
      return;
    }
    installing = true;
    setTestNote(selected, "");
    try {
      const next = await installAdapter(selected);
      installStatuses = { ...installStatuses, [selected]: next };
      settings = await getSettings();
    } catch (err) {
      const message = err instanceof Error ? err.message : "安装失败";
      setTestNote(selected, isPassiveSource(selected) ? message : `${message}。请使用下方“手动绑定”兜底。`);
    } finally {
      installing = false;
    }
  }

  async function setSourceEnabled(selected: AdapterSource, enabled: boolean) {
    if (copiedSource === selected) copiedSource = null;
    if (enabled) {
      await installSource(selected);
      return;
    }
    if (!settings) return;
    const adapters = { ...settings.adapters };
    if (selected === "codex") adapters.codex = false;
    if (selected === "cursor") adapters.cursor = false;
    if (selected === "grok-cli") adapters.grokCli = false;
    if (selected === "gemini-cli") adapters.geminiCli = false;
    if (selected === "workbuddy") adapters.workBuddy = false;
    if (selected === "marvis") adapters.marvis = false;
    if (selected === "dsh-desktop") adapters.dshDesktop = false;
    await patch({ adapters });
  }

  function sourceInstalled(selected: AdapterSource): boolean {
    return installStatuses[selected]?.installed === true;
  }

  function sourceEnabled(selected: AdapterSource): boolean {
    if (!settings || !sourceInstalled(selected)) return false;
    if (selected === "codex") return settings.adapters.codex;
    if (selected === "cursor") return settings.adapters.cursor;
    if (selected === "grok-cli") return settings.adapters.grokCli;
    if (selected === "gemini-cli") return settings.adapters.geminiCli;
    if (selected === "workbuddy") return settings.adapters.workBuddy;
    if (selected === "marvis") return settings.adapters.marvis;
    return settings.adapters.dshDesktop;
  }

  async function removeSource(selected: AdapterSource) {
    if (browserPreview) {
      installStatuses = { ...installStatuses, [selected]: previewInstallStatus(selected, false) };
      await setSourceEnabled(selected, false);
      return;
    }
    removing = true;
    setTestNote(selected, "");
    try {
      const next = await uninstallAdapter(selected);
      installStatuses = { ...installStatuses, [selected]: next };
      settings = await getSettings();
    } catch (err) {
      setTestNote(selected, err instanceof Error ? err.message : "移除失败");
    } finally {
      removing = false;
    }
  }

  function toggleAdapterDetails(selected: AdapterSource) {
    if (expandedSource === selected) {
      expandedSource = null;
      return;
    }
    expandedSource = selected;
    copiedSource = null;
    setTestNote(selected, "");
    void loadInstallStatus(selected);
  }

  function previewInstallStatus(selected: AdapterSource, installed: boolean): AdapterInstallStatus {
    const home = selected === "workbuddy"
      ? "~/.workbuddy/projects"
      : selected === "marvis"
        ? "~/.marvis/database/data.db"
        : selected === "dsh-desktop"
          ? "~/dsh-desktop/harness/storages/session_projcache.json"
          : selected === "grok-cli"
        ? "~/.grok/hooks/springcat.json"
        : selected === "gemini-cli"
          ? "~/.gemini/settings.json"
          : `~/.${selected}/hooks.json`;
    return {
      source: selected,
      installed,
      configPath: home,
      bridgeInstalled: !isPassiveSource(selected),
      requiresTrust: selected === "codex",
      message: installed ? "监听已连接，SpringCat 会接收任务生命周期事件。" : "尚未安装监听。",
    };
  }

  onMount(() => {
    document.documentElement.classList.add("settings");
    if (browserPreview) {
      settings = {
        ...DEFAULT_SETTINGS,
        doubleClickAction: "open-latest",
        monitorDocks: {},
      };
      bind = {
        inboxDir: "C:/Users/demo/AppData/Roaming/springcat-ai/inbox",
        bridgePath: "C:/Users/demo/AppData/Roaming/springcat-ai/bin/springcat-bridge.exe",
        bridgeFound: true,
      };
      storageInfo = {
        defaultDirectory: "C:/Users/demo/AppData/Roaming/springcat-ai",
        activeDirectory: "C:/Users/demo/AppData/Roaming/springcat-ai",
        restartRequired: false,
      };
      browserInfo = {
        systemDefaultName: "Google Chrome",
        systemDefaultPath: "C:/Program Files/Google/Chrome/Application/chrome.exe",
        browsers: [
          { name: "Google Chrome", path: "C:/Program Files/Google/Chrome/Application/chrome.exe" },
          { name: "Microsoft Edge", path: "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe" },
        ],
      };
      installStatuses = Object.fromEntries(
        ADAPTERS.map((adapter) => [adapter.id, previewInstallStatus(adapter.id, true)]),
      ) as InstallStatusMap;
    } else {
      void getSettings().then((value) => {
        settings = value;
      });
      void adapterBindInfo().then((value) => {
        bind = value;
      });
      void getStorageInfo().then((value) => {
        storageInfo = value;
      });
      void getBrowserInfo().then((value) => {
        browserInfo = value;
      });
      for (const adapter of ADAPTERS) void loadInstallStatus(adapter.id);
    }
    return () => document.documentElement.classList.remove("settings");
  });
</script>

{#if settings}
  <div class="settings-shell">
    <aside class="sidebar">
      <div class="brand-lockup">
        <span class="brand-mark" aria-hidden="true">
          <img src={brandLogo} alt="" />
        </span>
        <span><strong>SpringCat</strong><small>控制中心</small></span>
      </div>

      <nav aria-label="设置菜单">
        <span class="nav-label">设置</span>
        <button type="button" class:active={tab === "general"} onclick={() => (tab = "general")}>
          <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M10 3.3 16 6.7v6.6l-6 3.4-6-3.4V6.7l6-3.4Z" /><circle cx="10" cy="10" r="2.2" /></svg>
          <span><strong>常规</strong><small>外观、行为和存储</small></span>
        </button>
        <button type="button" class:active={tab === "adapters"} onclick={() => (tab = "adapters")}>
          <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M7 4v4m6-4v4M5 8h10v2a5 5 0 0 1-5 5 5 5 0 0 1-5-5V8Zm5 7v2" /></svg>
          <span><strong>AI 工具</strong><small>来源绑定与监听</small></span>
        </button>
        <span class="nav-label spacing">洞察</span>
        <button type="button" class:active={tab === "usage"} onclick={() => (tab = "usage")}>
          <svg viewBox="0 0 20 20" aria-hidden="true"><rect x="3" y="4.5" width="14" height="12" rx="2" /><path d="M6 3v3m8-3v3M3 8h14m-10 3h2m2 0h2m-6 3h2m2 0h2" /></svg>
          <span><strong>用量日历</strong><small>每日 Token 消耗</small></span>
          <em>新</em>
        </button>
      </nav>

      <div class="sidebar-footer">
        <span class="privacy-dot"></span>
        <p><strong>本地优先</strong><small>设置和统计保存在此设备</small></p>
      </div>
    </aside>

    <main>
      <header class="page-header">
        <div>
          <span class="eyebrow">{tab === "usage" ? "INSIGHTS" : "PREFERENCES"}</span>
          <h1>{tab === "general" ? "常规设置" : tab === "adapters" ? "AI 工具" : "用量日历"}</h1>
          <p>
            {tab === "general"
              ? "调整 SpringCat 的展示、交互与本地存储。"
              : tab === "adapters"
                ? "连接你的 AI 工具，只监听任务状态，不保存完整对话。"
                : "按天查看已绑定 AI 工具的 Token 使用情况。"}
          </p>
        </div>
        {#if saving}<span class="saving-indicator"><i></i>正在保存</span>{/if}
      </header>

      <div class="page-scroll">
        <div class:wide={tab === "usage"} class="page-content">
          {#if tab === "general"}
            <SettingsCard icon="eye" title="展示" description="控制工作面板在桌面上的呈现方式。">
              <div class="field-list">
                <SelectField
                  label="展示模式"
                  hint="选择常驻桌面的主要形态"
                  value={settings.presentationMode}
                  options={[
                    { value: "work", label: "工作面板" },
                    { value: "pet", label: "宠物模式（即将推出）", disabled: true },
                  ]}
                  onchange={(value) => void patch({ presentationMode: value === "pet" ? "pet" : "work" })}
                />
                <SelectField
                  label="默认吸附边"
                  hint="首次启动或没有位置记录时使用"
                  value={settings.dockSide}
                  options={[
                    { value: "top", label: "顶部" },
                    { value: "left", label: "左侧" },
                    { value: "right", label: "右侧" },
                  ]}
                  onchange={(value) => void patch({ dockSide: value as DockSide })}
                />
              </div>
            </SettingsCard>

            <SettingsCard icon="activity" title="行为" description="设置启动方式、提醒节奏和双击操作。">
              <div class="field-list">
                <ToggleRow
                  label="兼容灵动岛"
                  hint="置顶时拉长胶囊，并将任务文字移到右侧以避开屏幕中央区域"
                  checked={settings.dynamicIslandCompatible}
                  disabled={saving}
                  onchange={(checked) => void patch({ dynamicIslandCompatible: checked })}
                />
                <ToggleRow
                  label="顶部吸附"
                  hint="吸附到屏幕顶部中央；关闭后可停靠边缘，悬浮球仍保持可见"
                  checked={settings.alwaysOnTop}
                  onchange={(checked) => void patch({ alwaysOnTop: checked })}
                />
                <ToggleRow
                  label="执行时自动置顶"
                  hint="有对话正在执行时临时吸附到顶部，全部结束后恢复手动置顶状态"
                  checked={settings.autoPinWhileRunning}
                  onchange={(checked) => void patch({ autoPinWhileRunning: checked })}
                />
                <ToggleRow
                  label="开机启动"
                  hint="登录系统后自动启动 SpringCat"
                  checked={settings.autostart}
                  onchange={(checked) => void patch({ autostart: checked })}
                />
                <ToggleRow
                  label="专注模式"
                  hint="完成任务只保留未读点，减少打扰"
                  checked={settings.focusMode}
                  onchange={(checked) => void patch({ focusMode: checked })}
                />
                <SelectField
                  label="双击面板"
                  hint="选择双击工作面板时执行的操作"
                  value={settings.doubleClickAction}
                  options={[
                    { value: "open-latest", label: "打开最近待处理任务" },
                    { value: "none", label: "无操作" },
                  ]}
                  onchange={(value) => void patch({ doubleClickAction: value as DoubleClickAction })}
                />
                <SelectField
                  label="外部链接浏览器"
                  hint="默认跟随 Windows；仅影响网页链接，不改变应用内设置窗口"
                  ariaLabel="外部链接浏览器"
                  value={settings.browserPath ?? ""}
                  options={browserOptions}
                  onchange={(value) => void patch({ browserPath: value })}
                >
                  {#snippet extra()}
                    <Button compact disabled={saving || choosingBrowser} onclick={() => void chooseBrowserExecutable()}>
                      {choosingBrowser ? "选择中…" : "选择程序"}
                    </Button>
                  {/snippet}
                </SelectField>
                <SelectField
                  label="任务历史保留"
                  hint="Token 统计将使用独立的保留策略"
                  value={String(settings.historyRetentionDays)}
                  options={[
                    { value: "1", label: "1 天" },
                    { value: "7", label: "7 天" },
                    { value: "30", label: "30 天" },
                    { value: "0", label: "不保存" },
                  ]}
                  onchange={(value) => void patch({ historyRetentionDays: Number(value) })}
                />
              </div>
            </SettingsCard>

            <SettingsCard icon="layers" title="本地存储" description="管理任务历史、事件收件箱和日志的保存位置。">
              <div class="storage-control">
                <input readonly value={storageDirectory} aria-label="缓存数据位置" />
                <Button disabled={saving || choosingStorage} onclick={() => void chooseStorageDirectory()}>
                  {choosingStorage ? "选择中…" : "选择目录"}
                </Button>
              </div>
              <div class="card-actions">
                <Button variant="text" disabled={saving || !storageInfo?.configuredDirectory} onclick={() => void applyStorageDirectory("")}>恢复默认位置</Button>
                {#if storageInfo?.restartRequired}
                  <Button variant="primary" disabled={saving} onclick={() => void restartApp()}>立即重启</Button>
                {/if}
              </div>
              {#if storageInfo?.restartRequired}<p class="note">当前仍在使用：{storageInfo.activeDirectory}</p>{/if}
              {#if storageNote}<p class="note">{storageNote}</p>{/if}
            </SettingsCard>
          {:else if tab === "adapters"}
            <SettingsCard icon="plug" title="来源绑定" description="打开开关会同时安装监听并启用来源。">
              <div class="adapter-list">
                {#each ADAPTERS as adapter}
                  {@const installStatus = installStatuses[adapter.id] ?? null}
                  <div class:expanded={expandedSource === adapter.id} class="adapter-item">
                    <div class="adapter-row">
                      <span class="adapter-logo source-{adapter.id}"><ToolLogo source={adapter.id} /></span>
                      <span class="adapter-copy"><strong>{adapter.label}</strong><small>{adapter.detail}</small></span>
                      <span class:online={sourceInstalled(adapter.id)} class="adapter-status"><i></i>{sourceInstalled(adapter.id) ? isPassiveSource(adapter.id) ? "监听中" : "已绑定" : "未绑定"}</span>
                      <Toggle
                        checked={sourceEnabled(adapter.id)}
                        disabled={installing || removing}
                        ariaLabel={`${sourceEnabled(adapter.id) ? "关闭" : "启用"}${adapter.label}监听`}
                        onchange={(checked) => void setSourceEnabled(adapter.id, checked)}
                      />
                      <button
                        class="adapter-expand"
                        type="button"
                        aria-expanded={expandedSource === adapter.id}
                        aria-controls={`adapter-details-${adapter.id}`}
                        onclick={() => toggleAdapterDetails(adapter.id)}
                      >
                        <span>{expandedSource === adapter.id ? "收起" : "详情"}</span>
                        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m4.5 6 3.5 3.5L11.5 6" /></svg>
                      </button>
                    </div>

                    {#if expandedSource === adapter.id}
                      <div class="adapter-detail" id={`adapter-details-${adapter.id}`}>
                        <div class="connection-panel">
                          <div class="connection-heading">
                            <span class:online={installStatus?.installed} class="status-dot"></span>
                            <div>
                              <strong>{isPassiveSource(adapter.id) ? installStatus?.installed ? "已检测到本地会话" : `未检测到 ${adapter.label}` : installStatus?.installed ? "监听工作正常" : "尚未绑定"}</strong>
                              <p>{installStatus?.message ?? "正在检查 hooks 配置…"}</p>
                            </div>
                          </div>
                          {#if installStatus?.requiresTrust}<p class="note">Codex 本机会话由 SpringCat 直接监听；在 <code>/hooks</code> 中确认信任可启用额外实时通道。</p>{/if}
                          {#if adapter.id === "grok-cli" && installStatus?.installed}<p class="note">Grok 全局 hook 无需项目信任；绑定后请新建或重启一次 Grok 会话。</p>{/if}
                          {#if adapter.id === "gemini-cli" && installStatus?.installed}<p class="note">Gemini CLI 使用官方全局 hooks；现有认证与安全设置会原样保留，绑定后请新建或重启一次会话。</p>{/if}
                          {#if adapter.id === "workbuddy" && installStatus?.installed}<p class="note">直接只读监听本地 JSONL，不会把完整对话、推理和工具输出写入 SpringCat。</p>{/if}
                          {#if adapter.id === "marvis" && installStatus?.installed}<p class="note">直接只读监听本地 SQLite/WAL，只保存生命周期、短标题、最终摘要和 Token 数字。</p>{/if}
                          <div class="card-actions">
                            <Button variant="primary" disabled={installing || removing} onclick={() => void installSource(adapter.id)}>
                              {installing ? "安装中…" : isPassiveSource(adapter.id) ? installStatus?.installed ? "重新检测" : `检测 ${adapter.label}` : installStatus?.installed ? "修复安装" : "启用监听"}
                            </Button>
                            {#if installStatus?.installed && !isPassiveSource(adapter.id)}
                              <Button variant="secondary" danger disabled={installing || removing} onclick={() => void removeSource(adapter.id)}>{removing ? "移除中…" : "移除监听"}</Button>
                            {/if}
                            <Button disabled={testing || !installStatus?.installed} onclick={() => void runTest(adapter.id)}>{testing ? "测试中…" : "测试连接"}</Button>
                          </div>
                        </div>
                        {#if installStatus}
                          <p class="meta">{isPassiveSource(adapter.id) ? "本地数据" : "hooks"}：{installStatus.configPath}{#if !isPassiveSource(adapter.id)}<br />bridge：{installStatus.bridgeInstalled ? "已安装" : "缺失"}{/if}</p>
                        {/if}
                        {#if bind && !isPassiveSource(adapter.id)}
                          <details class="fallback">
                            <summary>自动绑定失败？查看手动配置</summary>
                            <p class="note">复制配置并合并到上面的 hooks 路径。Cursor 保存后会自动重载，Grok 需要新建或重启会话。</p>
                            <textarea readonly rows="10">{bindPrompt(adapter.id, bind.bridgePath, bind.inboxDir)}</textarea>
                            <Button onclick={() => void copyPrompt(adapter.id)}>{copiedSource === adapter.id ? "已复制配置" : "复制手动配置"}</Button>
                          </details>
                        {/if}
                        {#if bind}<p class="meta">inbox：{bind.inboxDir}</p>{/if}
                        {#if testNotes[adapter.id]}<p class="note result-note">{testNotes[adapter.id]}</p>{/if}
                      </div>
                    {/if}
                  </div>
                {/each}
              </div>
            </SettingsCard>
          {:else}
            <UsageCalendar boundSources={boundUsageSources} preview={browserPreview} />
          {/if}

          <footer class="page-footer">
            <span>所有更改都会自动保存</span>
            <div>
              <Button disabled={saving} onclick={() => window.close()}>关闭窗口</Button>
              <Button variant="text" danger onclick={() => (browserPreview ? undefined : void quitApp())}>退出应用</Button>
            </div>
          </footer>
        </div>
      </div>
    </main>
  </div>
{:else}
  <div class="loading-state"><span></span><p>正在读取设置…</p></div>
{/if}

<style>
  .settings-shell {
    display: grid;
    grid-template-columns: 214px minmax(0, 1fr);
    height: 100vh;
    overflow: hidden;
    color: var(--sc-text);
    background: var(--settings-background);
  }

  .sidebar {
    display: flex;
    min-height: 0;
    flex-direction: column;
    padding: 22px 14px 16px;
    border-right: 1px solid var(--settings-border);
    background:
      radial-gradient(circle at 20% 0%, color-mix(in srgb, var(--settings-accent) 11%, transparent), transparent 34%),
      var(--settings-sidebar);
  }

  .brand-lockup {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 8px 24px;
  }

  .brand-mark {
    display: grid;
    width: 38px;
    height: 38px;
    flex: 0 0 38px;
    place-items: center;
    overflow: hidden;
    border: 1px solid var(--settings-border-strong);
    border-radius: 12px;
    background: var(--settings-logo-tile);
    box-shadow: 0 8px 20px color-mix(in srgb, var(--settings-accent) 12%, transparent);
  }

  .brand-mark img {
    width: 32px;
    height: 32px;
    object-fit: contain;
    filter: var(--settings-logo-filter);
  }

  .brand-lockup > span:last-child,
  .brand-lockup small,
  .nav-label,
  nav button span,
  nav button small,
  .sidebar-footer p,
  .sidebar-footer small,
  .adapter-copy,
  .adapter-copy small {
    display: block;
  }

  .brand-lockup strong {
    font-size: 14px;
    letter-spacing: -0.01em;
  }

  .brand-lockup small {
    margin-top: 1px;
    color: var(--sc-muted);
    font-size: 9px;
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .nav-label {
    padding: 0 10px 4px;
    color: color-mix(in srgb, var(--sc-muted) 72%, transparent);
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.09em;
    text-transform: uppercase;
  }

  .nav-label.spacing {
    padding-top: 13px;
  }

  nav button {
    position: relative;
    display: grid;
    grid-template-columns: 20px 1fr auto;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 9px 10px;
    border: 1px solid transparent;
    border-radius: 11px;
    background: transparent;
    color: var(--sc-muted);
    text-align: left;
    cursor: pointer;
  }

  nav button:hover {
    background: var(--settings-hover);
    color: var(--sc-text);
  }

  nav button.active {
    border-color: color-mix(in srgb, var(--settings-accent) 18%, transparent);
    background: var(--settings-accent-soft);
    color: var(--settings-accent);
  }

  nav button svg {
    width: 19px;
    fill: none;
    stroke: currentColor;
    stroke-linecap: round;
    stroke-linejoin: round;
    stroke-width: 1.55;
  }

  nav button strong {
    display: block;
    color: var(--sc-text);
    font-size: 11px;
    font-weight: 630;
  }

  nav button.active strong {
    color: var(--settings-accent);
  }

  nav button small {
    margin-top: 2px;
    color: var(--sc-muted);
    font-size: 8.5px;
  }

  nav button em {
    padding: 2px 5px;
    border-radius: 99px;
    background: var(--settings-accent);
    color: var(--settings-accent-text);
    font-size: 8px;
    font-style: normal;
    font-weight: 700;
  }

  .sidebar-footer {
    display: flex;
    align-items: center;
    gap: 9px;
    margin-top: auto;
    padding: 12px 9px 2px;
    border-top: 1px solid var(--settings-border);
  }

  .privacy-dot {
    width: 8px;
    height: 8px;
    border-radius: 99px;
    background: #42b883;
    box-shadow: 0 0 0 4px color-mix(in srgb, #42b883 13%, transparent);
  }

  .sidebar-footer p {
    margin: 0;
  }

  .sidebar-footer strong {
    font-size: 9px;
  }

  .sidebar-footer small {
    margin-top: 2px;
    color: var(--sc-muted);
    font-size: 8px;
  }

  main {
    display: flex;
    min-width: 0;
    min-height: 0;
    flex-direction: column;
  }

  .page-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 24px 30px 18px;
    border-bottom: 1px solid var(--settings-border);
    background: color-mix(in srgb, var(--settings-background) 92%, transparent);
  }

  .eyebrow {
    color: var(--settings-accent);
    font-size: 8.5px;
    font-weight: 750;
    letter-spacing: 0.13em;
  }

  h1 {
    margin: 4px 0 3px;
    font-size: 21px;
    line-height: 1.1;
    letter-spacing: -0.025em;
  }

  .page-header p {
    margin: 0;
    color: var(--sc-muted);
    font-size: 10px;
  }

  .saving-indicator {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 9px;
    border-radius: 99px;
    background: var(--settings-surface);
    color: var(--sc-muted);
    font-size: 9px;
  }

  .saving-indicator i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--settings-accent);
  }

  .page-scroll {
    min-height: 0;
    flex: 1;
    overflow: auto;
    scrollbar-color: color-mix(in srgb, var(--sc-muted) 34%, transparent) transparent;
  }

  .page-content {
    width: min(100%, 760px);
    margin: 0 auto;
    padding: 20px 30px 28px;
  }

  .page-content.wide {
    width: min(100%, 920px);
  }

  .field-list {
    border-top: 1px solid var(--settings-border);
  }

  .storage-control {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 8px;
    padding-top: 4px;
  }

  .storage-control input {
    width: 100%;
    height: 36px;
    padding: 0 11px;
    border: 1px solid var(--settings-border);
    border-radius: 10px;
    outline: none;
    background: var(--settings-control);
    color: var(--sc-muted);
    font: inherit;
  }

  .storage-control input:focus {
    border-color: var(--settings-accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--settings-accent) 12%, transparent);
  }

  textarea {
    font: inherit;
  }

  .card-actions,
  .page-footer > div {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .card-actions {
    margin-top: 12px;
  }

  .adapter-list {
    display: grid;
    gap: 7px;
    padding-top: 2px;
  }

  .adapter-item {
    overflow: hidden;
    border: 1px solid var(--settings-border);
    border-radius: 12px;
    background: var(--settings-surface);
    transition:
      border-color 150ms ease,
      box-shadow 150ms ease;
  }

  .adapter-item:hover,
  .adapter-item.expanded {
    border-color: color-mix(in srgb, var(--settings-accent) 30%, var(--settings-border));
  }

  .adapter-item.expanded {
    box-shadow: 0 8px 22px rgb(0 0 0 / 6%);
  }

  .adapter-row {
    display: grid;
    grid-template-columns: 36px minmax(0, 1fr) auto auto 58px;
    align-items: center;
    gap: 10px;
    min-height: 54px;
    padding: 8px 10px;
  }

  .adapter-logo {
    display: grid;
    box-sizing: border-box;
    width: 32px;
    height: 32px;
    padding: 7px;
    place-items: center;
    border-radius: 9px;
    background: var(--settings-control);
    color: var(--sc-text);
  }

  .source-codex { background: color-mix(in srgb, var(--usage-codex) 13%, var(--settings-control)); color: var(--usage-codex); }
  .source-cursor { background: color-mix(in srgb, var(--usage-cursor) 13%, var(--settings-control)); color: var(--usage-cursor); }
  .source-grok-cli { background: color-mix(in srgb, var(--usage-grok) 13%, var(--settings-control)); color: var(--usage-grok); }
  .source-gemini-cli { background: color-mix(in srgb, #8ab4f8 16%, var(--settings-control)); color: #4f86e8; }
  .source-workbuddy { background: color-mix(in srgb, #42b883 13%, var(--settings-control)); color: #2f9b70; }
  .source-marvis { background: color-mix(in srgb, var(--usage-marvis) 13%, var(--settings-control)); color: var(--usage-marvis); }

  .adapter-copy strong {
    font-size: 10.5px;
  }

  .adapter-copy small {
    margin-top: 2px;
    color: var(--sc-muted);
    font-size: 8.5px;
  }

  .adapter-status {
    display: flex;
    align-items: center;
    gap: 5px;
    color: var(--sc-muted);
    font-size: 8.5px;
  }

  .adapter-status i {
    width: 6px;
    height: 6px;
    border-radius: 99px;
    background: currentColor;
  }

  .adapter-status.online {
    color: var(--settings-online);
  }

  .adapter-expand {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 3px;
    min-height: 30px;
    padding: 5px 7px;
    border: 1px solid var(--settings-border);
    border-radius: 8px;
    background: var(--settings-control);
    color: var(--sc-muted);
    font-size: 8.5px;
    cursor: pointer;
  }

  .adapter-expand:hover,
  .adapter-expand:focus-visible,
  .adapter-item.expanded .adapter-expand {
    border-color: color-mix(in srgb, var(--settings-accent) 35%, var(--settings-border));
    color: var(--settings-accent);
  }

  .adapter-expand:focus-visible {
    outline: none;
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--settings-accent) 12%, transparent);
  }

  .adapter-expand svg {
    width: 13px;
    fill: none;
    stroke: currentColor;
    stroke-linecap: round;
    stroke-linejoin: round;
    stroke-width: 1.5;
    transition: transform 160ms ease;
  }

  .adapter-item.expanded .adapter-expand svg {
    transform: rotate(180deg);
  }

  .adapter-detail {
    padding: 13px;
    border-top: 1px solid var(--settings-border);
    background: color-mix(in srgb, var(--settings-card) 58%, var(--settings-surface));
    animation: detail-reveal 150ms ease-out both;
  }

  .connection-panel {
    padding: 13px;
    border-radius: 12px;
    background: var(--settings-control);
  }

  .connection-heading {
    display: flex;
    align-items: flex-start;
    gap: 9px;
  }

  .connection-heading strong {
    font-size: 10.5px;
  }

  .connection-heading p {
    margin: 3px 0 0;
    color: var(--sc-muted);
    font-size: 9px;
  }

  .status-dot {
    width: 8px;
    height: 8px;
    margin-top: 3px;
    border-radius: 999px;
    background: var(--sc-muted);
  }

  .status-dot.online {
    background: var(--settings-online);
    box-shadow: 0 0 0 4px color-mix(in srgb, var(--settings-online) 14%, transparent);
  }

  .note,
  .meta {
    color: var(--sc-muted);
    font-size: 9px;
    line-height: 1.5;
  }

  .meta {
    word-break: break-all;
  }

  .result-note {
    padding: 8px 10px;
    border-radius: 8px;
    background: var(--settings-accent-soft);
    color: var(--settings-accent);
  }

  code {
    padding: 1px 4px;
    border-radius: 4px;
    background: var(--settings-control);
  }

  .fallback {
    margin-top: 12px;
  }

  .fallback summary {
    color: var(--settings-accent);
    font-size: 9px;
    cursor: pointer;
  }

  textarea {
    min-height: 130px;
    margin: 7px 0;
    padding: 10px;
    resize: vertical;
    font-family: "Cascadia Mono", Consolas, monospace;
    font-size: 8.5px;
    line-height: 1.5;
    user-select: text;
  }

  .page-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 20px;
    padding-top: 14px;
    border-top: 1px solid var(--settings-border);
    color: var(--sc-muted);
    font-size: 9px;
  }

  .loading-state {
    display: grid;
    height: 100vh;
    place-content: center;
    place-items: center;
    gap: 10px;
    color: var(--sc-muted);
  }

  .loading-state span {
    width: 22px;
    height: 22px;
    border: 2px solid var(--settings-border);
    border-top-color: var(--settings-accent);
    border-radius: 50%;
    animation: spin 700ms linear infinite;
  }

  .loading-state p {
    margin: 0;
    font-size: 10px;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  @keyframes detail-reveal {
    from { opacity: 0; transform: translateY(-4px); }
    to { opacity: 1; transform: translateY(0); }
  }

  @media (max-width: 760px) {
    .settings-shell {
      grid-template-columns: 180px minmax(0, 1fr);
    }

    .page-header,
    .page-content {
      padding-left: 20px;
      padding-right: 20px;
    }

    .adapter-row {
      grid-template-columns: 36px minmax(0, 1fr) auto 52px;
    }

    .adapter-status {
      display: none;
    }
  }
</style>
