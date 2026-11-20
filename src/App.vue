<script setup lang="ts">
import {computed, onMounted, ref} from "vue";
import {
  darkTheme,
  GlobalThemeOverrides,
  lightTheme,
  NIcon,
} from "naive-ui";
import {open} from "@tauri-apps/plugin-dialog";
import {openUrl} from "@tauri-apps/plugin-opener";
import {FolderOpenOutline, RefreshOutline, SettingsOutline} from "@vicons/ionicons5";
import FileTree from "./FileTree.vue";
import {eventBus} from "./component/eventBus.ts";
import packageInfo from "../package.json";

const themePresets = [
  {key: "cloud", name: "云白", mode: "light", accent: "#16836f", accentHover: "#116c5c", accentPressed: "#0c594c", background: "#f3f6f5", surface: "#ffffff", surfaceRaised: "#f8faf9", text: "#202a28", textMuted: "#65716e", border: "#dce4e1", scrollbar: "rgba(60, 91, 82, .28)", scrollbarHover: "rgba(60, 91, 82, .48)"},
  {key: "mist", name: "雾青", mode: "light", accent: "#256b78", accentHover: "#1d5864", accentPressed: "#174751", background: "#f1f5f6", surface: "#ffffff", surfaceRaised: "#f6f9f9", text: "#202a2d", textMuted: "#647276", border: "#dbe4e6", scrollbar: "rgba(62, 100, 108, .28)", scrollbarHover: "rgba(62, 100, 108, .48)"},
  {key: "graphite", name: "石墨", mode: "dark", accent: "#65c6a5", accentHover: "#82d4b7", accentPressed: "#4dae8e", background: "#171b1b", surface: "#202625", surfaceRaised: "#272e2d", text: "#e5eeeb", textMuted: "#a0aeaa", border: "#343e3b", scrollbar: "rgba(156, 181, 172, .3)", scrollbarHover: "rgba(156, 181, 172, .52)"},
  {key: "deep-sea", name: "深海", mode: "dark", accent: "#71b9d0", accentHover: "#8bc9dc", accentPressed: "#57a1b9", background: "#151b20", surface: "#1e272d", surfaceRaised: "#26323a", text: "#e4edf0", textMuted: "#9babb2", border: "#34434b", scrollbar: "rgba(139, 173, 187, .3)", scrollbarHover: "rgba(139, 173, 187, .52)"}
] as const;

type ThemeKey = typeof themePresets[number]["key"];
const storageKey = "fsize.theme";
const savedTheme = localStorage.getItem(storageKey) as ThemeKey | null;
const themeKey = ref<ThemeKey>(themePresets.some(preset => preset.key === savedTheme) ? savedTheme! : "graphite");
const activePreset = computed(() => themePresets.find(preset => preset.key === themeKey.value)!);
const baseTheme = computed(() => activePreset.value.mode === "dark" ? darkTheme : lightTheme);
const themeVariables = computed(() => ({
  "--app-bg": activePreset.value.background,
  "--app-surface": activePreset.value.surface,
  "--app-surface-raised": activePreset.value.surfaceRaised,
  "--app-text": activePreset.value.text,
  "--app-text-muted": activePreset.value.textMuted,
  "--app-border": activePreset.value.border,
  "--app-accent": activePreset.value.accent,
  "--app-accent-soft": `${activePreset.value.accent}1a`,
  "--app-scrollbar": activePreset.value.scrollbar,
  "--app-scrollbar-hover": activePreset.value.scrollbarHover
}));
const themeOverrides = computed<GlobalThemeOverrides>(() => ({
  common: {
    primaryColor: activePreset.value.accent,
    primaryColorHover: activePreset.value.accentHover,
    primaryColorPressed: activePreset.value.accentPressed,
    bodyColor: activePreset.value.background,
    cardColor: activePreset.value.surface,
    modalColor: activePreset.value.surface,
    popoverColor: activePreset.value.surfaceRaised,
    textColorBase: activePreset.value.text,
    textColor1: activePreset.value.text,
    textColor2: activePreset.value.textMuted,
    borderColor: activePreset.value.border,
    dividerColor: activePreset.value.border,
    scrollbarColor: activePreset.value.scrollbar,
    scrollbarColorHover: activePreset.value.scrollbarHover
  }
}));

const fileTree = ref<InstanceType<typeof FileTree> | null>(null);
const currentPath = ref("");
const stateMessage = ref("选择一个目录开始分析");
const scanProgress = ref<{path: string; entriesScanned: number; elapsedMs: number} | null>(null);
const settingsOpen = ref(false);
const updateChecking = ref(false);
const latestRelease = ref<{version: string; url: string} | null>(null);
const updateError = ref("");
const updateStatus = ref("");

const formatElapsed = (milliseconds: number) => {
  const seconds = Math.floor(milliseconds / 1000);
  const minutes = Math.floor(seconds / 60);
  return minutes ? `${minutes}分${seconds % 60}秒` : `${seconds}秒`;
};

const normalizeVersion = (version: string) => version.replace(/^v/, "").split("-")[0].split(".").map(part => Number.parseInt(part, 10) || 0);

const isNewerVersion = (candidate: string, current: string) => {
  const latest = normalizeVersion(candidate);
  const installed = normalizeVersion(current);
  for (let index = 0; index < 3; index += 1) {
    if ((latest[index] ?? 0) !== (installed[index] ?? 0)) return (latest[index] ?? 0) > (installed[index] ?? 0);
  }
  return false;
};

const checkForUpdates = async () => {
  if (updateChecking.value) return;
  updateChecking.value = true;
  updateError.value = "";
  try {
    const response = await fetch("https://api.github.com/repos/ligson/fsize/releases/latest", {
      headers: {Accept: "application/vnd.github+json"}
    });
    if (!response.ok) throw new Error(`GitHub 返回 ${response.status}`);
    const release = await response.json() as {tag_name?: string; html_url?: string; draft?: boolean; prerelease?: boolean};
    if (!release.tag_name || !release.html_url || release.draft || release.prerelease) throw new Error("没有可用的正式版本");
    if (isNewerVersion(release.tag_name, packageInfo.version)) {
      latestRelease.value = {version: release.tag_name.replace(/^v/, ""), url: release.html_url};
      updateStatus.value = `发现新版本 v${latestRelease.value.version}`;
    } else {
      latestRelease.value = null;
      updateStatus.value = "当前已经是最新版本";
    }
  } catch (error) {
    updateError.value = String(error);
    updateStatus.value = "暂时无法检查更新，请稍后重试";
  } finally {
    updateChecking.value = false;
  }
};

const openLatestRelease = async () => {
  if (latestRelease.value) await openUrl(latestRelease.value.url);
};

const selectDirectory = async () => {
  try {
    const path = await open({multiple: false, directory: true, title: "选择要分析的目录"});
    if (typeof path === "string" && path) {
      currentPath.value = path;
      fileTree.value?.selectPath(path);
    }
  } catch (error) {
    stateMessage.value = `选择目录失败：${String(error)}`;
  }
};

const setTheme = (key: ThemeKey) => {
  themeKey.value = key;
  localStorage.setItem(storageKey, key);
};

onMounted(() => {
  eventBus.on("state-message", (event) => {
    stateMessage.value = event?.text ?? "";
    scanProgress.value = null;
  });
  eventBus.on("scan-progress", (event) => {
    scanProgress.value = event;
  });
  void checkForUpdates();
});
</script>

<template>
  <n-config-provider :theme="baseTheme" :theme-overrides="themeOverrides">
    <n-message-provider>
      <n-dialog-provider>
        <div class="app-shell" :class="`theme-${activePreset.mode}`" :style="themeVariables">
          <header class="app-toolbar">
            <nav class="menu-bar" aria-label="应用功能">
              <span class="active-workspace"><n-icon><FolderOpenOutline /></n-icon>空间分析</span>
            </nav>

            <div class="toolbar-spacer"></div>
            <n-button
              quaternary
              circle
              class="icon-button settings-button"
              title="设置"
              aria-label="设置"
              @click="settingsOpen = true"
            >
              <template #icon><n-icon><SettingsOutline /></n-icon></template>
            </n-button>
            <n-button type="primary" class="choose-button" @click="selectDirectory">
              <template #icon><n-icon><FolderOpenOutline /></n-icon></template>
              选择目录
            </n-button>
          </header>

          <main class="workspace">
            <div v-if="latestRelease" class="update-notice">
              <span>发现新版本 v{{ latestRelease.version }}</span>
              <n-button size="tiny" type="primary" @click="openLatestRelease">查看下载</n-button>
            </div>
            <section class="location-bar">
              <div class="location-icon"><n-icon><FolderOpenOutline /></n-icon></div>
              <div class="location-copy">
                <span class="location-caption">当前目录</span>
                <span class="location-path" :title="currentPath">{{ currentPath || "尚未选择目录" }}</span>
              </div>
              <n-button
                quaternary
                circle
                class="icon-button refresh-button"
                title="重新扫描"
                aria-label="重新扫描"
                :disabled="!currentPath"
                @click="fileTree?.selectPath(currentPath)"
              >
                <template #icon><n-icon><RefreshOutline /></n-icon></template>
              </n-button>
            </section>

            <section class="results-panel">
              <div class="results-heading">
                <div>
                  <h1>目录内容</h1>
                  <p>按占用空间排列</p>
                </div>
                <span class="theme-indicator">{{ activePreset.name }}主题</span>
              </div>
              <div class="tree-workspace">
                <FileTree ref="fileTree" />
              </div>
            </section>
          </main>

          <footer class="status-bar">
            <span class="status-dot" :class="{'status-busy': stateMessage.includes('正在')}"></span>
            <span class="status-text" :class="{'status-text-scanning': scanProgress}" :title="stateMessage">{{ stateMessage }}</span>
            <span v-if="scanProgress" class="status-scan-path" :title="scanProgress.path">{{ scanProgress.path }}</span>
            <span v-if="scanProgress" class="status-scan-meta">
              {{ scanProgress.entriesScanned.toLocaleString() }} 项 · {{ formatElapsed(scanProgress.elapsedMs) }}
            </span>
            <span class="status-theme">{{ activePreset.name }}</span>
            <span class="status-version">v{{ packageInfo.version }}</span>
          </footer>

          <n-drawer v-model:show="settingsOpen" :width="360" placement="right">
            <n-drawer-content title="设置" closable>
              <div class="settings-section">
                <div class="settings-section-heading">
                  <h2>外观</h2>
                  <span>主题</span>
                </div>
                <div class="theme-options">
                  <button
                    v-for="preset in themePresets"
                    :key="preset.key"
                    class="theme-option"
                    :class="{'theme-option-active': themeKey === preset.key}"
                    :aria-pressed="themeKey === preset.key"
                    @click="setTheme(preset.key)"
                  >
                    <span class="theme-preview" :style="{'--preview-bg': preset.background, '--preview-surface': preset.surface, '--preview-accent': preset.accent}">
                      <i></i><i></i><i></i>
                    </span>
                    <span class="theme-option-copy">
                      <strong>{{ preset.name }}</strong>
                      <small>{{ preset.mode === "dark" ? "暗色系" : "亮色系" }}</small>
                    </span>
                    <span class="theme-swatch" :style="{background: preset.accent}"></span>
                  </button>
                </div>
              </div>
              <div class="settings-section update-section">
                <div class="settings-section-heading">
                  <h2>更新</h2>
                  <span>GitHub Releases</span>
                </div>
                <p v-if="latestRelease" class="update-copy">发现新版本 v{{ latestRelease.version }}</p>
                <p v-else-if="updateError" class="update-copy">{{ updateStatus }}</p>
                <p v-else-if="updateStatus" class="update-copy">{{ updateStatus }}</p>
                <p v-else class="update-copy">当前版本 v{{ packageInfo.version }}</p>
                <div class="update-actions">
                  <n-button size="small" secondary :loading="updateChecking" @click="checkForUpdates()">检查更新</n-button>
                  <n-button v-if="latestRelease" size="small" type="primary" @click="openLatestRelease">打开下载页</n-button>
                </div>
                <small class="update-hint">目前自动检查并打开下载页；自动安装需要配置 Tauri updater 签名。</small>
              </div>
            </n-drawer-content>
          </n-drawer>
        </div>
      </n-dialog-provider>
    </n-message-provider>
  </n-config-provider>
</template>

<style>
@import "./assets/app.css";
</style>
