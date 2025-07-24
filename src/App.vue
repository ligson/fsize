<script setup lang="ts">
import {onMounted, onUnmounted, ref} from "vue";
import {darkTheme, GlobalTheme, lightTheme, NLayout} from "naive-ui"
import {open} from '@tauri-apps/plugin-dialog';
import FileTree from "./FileTree.vue";
import {eventBus} from "./component/eventBus.ts";


const myTheme = ref<GlobalTheme>(lightTheme);
const fileTree = ref<any>();
const currentSelectPath = ref<string>("选择路径:");
const stateMessage = ref<any>()
const selectFile = async () => {
  const path = await open({
    multiple: false,
    directory: true,
    title: '选择文件或者目录'
  });

  if (path) {
    if (fileTree.value) {
      currentSelectPath.value = "选择路径:" + path;
      fileTree.value.selectPath(path);
    }
  }
}

// 检测系统主题变化
const updateTheme = () => {
  const isDarkMode = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches;
  myTheme.value = isDarkMode ? darkTheme : lightTheme;
};


// 在组件挂载时添加监听器
onMounted(() => {
  eventBus.on('state-message', (event) => {
    stateMessage.value = event?.text;
  });
  updateTheme(); // 初始化主题
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', updateTheme);
});

// 在组件卸载时移除监听器
onUnmounted(() => {
  window.matchMedia('(prefers-color-scheme: dark)').removeEventListener('change', updateTheme);
});
</script>

<template>
  <n-config-provider :theme="myTheme" style="flex: 1;display: flex;flex-direction: column;">
    <n-message-provider>
      <n-dialog-provider>
        <n-layout class="x-layout">
          <n-layout-header class="x-layout-header">
            <n-space vertical style="flex: 1;padding: 10px;">
              <n-button type="primary" @click="selectFile">选择文件</n-button>
              <n-tag type="warning">{{ currentSelectPath }}</n-tag>
            </n-space>
          </n-layout-header>
          <n-layout-content class="x-layout-content">
            <FileTree ref="fileTree"/>
          </n-layout-content>
          <n-layout-footer class="x-layout-footer">{{ stateMessage }}</n-layout-footer>
        </n-layout>
      </n-dialog-provider>
    </n-message-provider>
  </n-config-provider>
</template>

<style>
@import "./assets/app.css";
</style>
