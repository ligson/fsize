<script lang="ts" setup>
import {ref} from "vue";
import {invoke} from "@tauri-apps/api/core";
import {TreeOption, useDialog, useMessage} from "naive-ui";
import TreeNodeContextMenu from "./TreeNodeContextMenu.vue";
import {listen} from '@tauri-apps/api/event';


const treeData = ref<TreeNode[]>([]);

const currentSelectPath = ref<string | number | undefined>('');
const currentPath = ref<string>("");
const message = useMessage();
const treeNodeContextMenuRef = ref<any>();
const treeLoading = ref<boolean>(false);
const dialog = useDialog()

interface TreeNode {
  label: string;
  key: string;
  type: 'directory' | 'file';
  icon: string; // 添加图标字段
  expandable: boolean; // 添加是否可展开字段
  children?: TreeNode[];
}

const formatSize = (size: number): string => {
  if (size === 0) {
    return '0 B';
  }
  const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB', 'EB', 'ZB', 'YB'];
  let i = Math.floor(Math.log(size) / Math.log(1024));

  // 调整 i 的值以满足你的显示要求
  if (i >= 3) { // 如果 size 大于等于 GB 的大小，则显示为 MB
    i = 2;
  } else if (i >= 2) { // 如果 size 大于等于 MB 的大小但小于 GB，则显示为 MB
    i = 2;
  } else if (i >= 1) { // 如果 size 大于等于 KB 的大小但小于 MB，则显示为 KB
    i = 1;
  }

  return `${(size / Math.pow(1024, i)).toFixed(2)} ${units[i]}`;
}

const convertToTree = async (node: any): Promise<TreeNode> => {
  const label: string = "[" + formatSize(node.size) + "]-" + node.name;
  const isDirectory: boolean = node.is_dir;

  let children: TreeNode[] | undefined = undefined;
  if (node.children) {
    children = await Promise.all(Object.values(node.children).map(convertToTree));
  }

  const treeNode: TreeNode = {
    label: label,
    key: node.path,
    type: isDirectory ? 'directory' : 'file',
    icon: isDirectory ? 'folder-icon' : 'file-icon', // 设置不同的图标
    expandable: isDirectory, // 只有目录可以展开
    children: children
  };
  return treeNode;
};

const processTreeData = async (fileSizeResult: any) => {
  console.log(typeof fileSizeResult)
  console.log(new Date().toLocaleString())
  if (typeof fileSizeResult === 'string') {
    const result = JSON.parse(fileSizeResult);
    convertToTree(result).then(treeNode => {
      treeData.value = [treeNode];
    });
  } else {
    console.error('Error:', fileSizeResult);
  }
}


// 监听 file_size_result 事件
listen('file_size_result', (event) => {
  const payload = event.payload as string | { error: string };
  processTreeData(payload).then(_ => {
    treeLoading.value = false;
  })

});
listen('file_deleted', (_) => {
  message.success("删除" + currentSelectPath.value + "成功")
  selectPath(currentPath.value)
})
listen('file_deleted_error', (event) => {
  const payload = event.payload as string | { error: string };
  message.error("删除" + currentSelectPath.value + "失败:" + payload)
})
const selectPath = (path: string) => {
  currentPath.value = path;
  treeData.value = [];
  console.log(new Date().toLocaleString())
  treeLoading.value = true;
  // 调用 read_file 命令
  invoke('file_size', {path: path})
      .then(fileSizeResult => {
        console.log(fileSizeResult);
      })
      .catch(err => console.error(err));
}


const nodeProps = ({option}: { option: TreeOption }) => {
  return {
    onClick() {
      console.log(message)
      message.info("[Click]" + option.label)
    },
    onContextmenu(e: MouseEvent): void {
      currentSelectPath.value = option.key
      if (treeNodeContextMenuRef.value) {
        treeNodeContextMenuRef.value.showContextMenu(e)
      }

    }
  }
}
const fileDeleteClick = () => {
  dialog.warning({
    title: '确认删除',
    content: '确认删除文件：' + currentSelectPath.value,
    positiveText: '确定',
    negativeText: '不确定',
    onPositiveClick: () => {
      invoke('file_delete', {path: currentSelectPath.value})
    }
  })
}
const selectMenu = (item: string) => {
  if (item === "refresh") {
    selectPath(currentPath.value);
  } else if (item === "delete") {
    fileDeleteClick();
  }
}

defineExpose({selectPath})
</script>
<template>
  <n-spin :show="treeLoading">
    <n-tree :data="treeData" block-line :node-props="nodeProps"
    />
    <TreeNodeContextMenu ref="treeNodeContextMenuRef" @select="selectMenu"/>
  </n-spin>
</template>
