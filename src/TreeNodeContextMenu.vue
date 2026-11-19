<script setup lang="ts">
import {ref} from "vue";
import {useMessage} from "naive-ui";

const message = useMessage();
const dropMenuXRef = ref(0);
const dropMenuYRef = ref(0);
const showDropdown = ref(false);

const emit = defineEmits(['select'])
const selectDropdown = (key: string | number) => {
  message.info(String(key))
  showDropdown.value = false;
  emit('select', String(key))
}

const clickOutside = () => {
  showDropdown.value = false;
}
const dropdownOptions = [
  {label: '刷新', key: 'refresh'},
  {label: '删除', key: 'delete'},
  {label: '在文件管理器中定位', key: 'reveal'},
  {label: '打开', key: 'open'}];

const showContextMenu = (event: MouseEvent) => {
  event.preventDefault()
  showDropdown.value = true;
  dropMenuXRef.value = event.clientX;
  dropMenuYRef.value = event.clientY;
}
defineExpose({showContextMenu});
</script>

<template>
  <n-dropdown trigger="manual" placement="bottom-start"
              :options="dropdownOptions"
              :show="showDropdown"
              @select="selectDropdown"
              :x="dropMenuXRef" :y="dropMenuYRef"
              @clickoutside="clickOutside">
  </n-dropdown>
</template>

<style scoped>

</style>
