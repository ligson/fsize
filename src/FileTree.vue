<script lang="ts" setup>
import {computed, h, ref, watch} from "vue";
import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import {NCheckbox, NIcon, TreeOption, useDialog, useMessage} from "naive-ui";
import {AddOutline, DocumentTextOutline, FolderOpenOutline, FolderOutline, LockClosedOutline, SearchOutline} from "@vicons/ionicons5";
import {open} from "@tauri-apps/plugin-dialog";
import TreeNodeContextMenu from "./TreeNodeContextMenu.vue";
import {eventBus} from "./component/eventBus.ts";
import {MyTool} from "./util/MyTool.ts";

interface TreeNode extends TreeOption {
  label: string;
  key: string;
  type: "directory" | "file";
  readable: boolean;
  complete: boolean;
  size: number;
  sizeLabel: string;
  searchable: string;
  hasChildren: boolean;
  loaded: boolean;
  isLoadMore?: boolean;
  parentPath?: string;
  nextOffset?: number;
  totalChildren?: number;
  loadingMore?: boolean;
  children?: TreeNode[];
}

interface FileSizeEntry {
  name: string;
  path: string;
  size: number;
  is_dir: boolean;
  readable: boolean;
  complete: boolean;
  has_children: boolean;
}

interface ScanSummary {
  root: FileSizeEntry;
  item_count: number;
  incomplete_count: number;
}

interface ChildrenPage {
  entries: FileSizeEntry[];
  offset: number;
  total: number;
  has_more: boolean;
}

interface ScanProgressEvent {
  path: string;
  entries_scanned: number;
}

const treeData = ref<TreeNode[]>([]);
const selectedKeys = ref<Array<string | number>>([]);
const currentSelectPath = ref("");
const currentPath = ref("");
const message = useMessage();
const dialog = useDialog();
const treeNodeContextMenuRef = ref<InstanceType<typeof TreeNodeContextMenu> | null>(null);
const treeLoading = ref(false);
const scanReady = ref(false);
const itemCount = ref(0);
const incompleteCount = ref(0);
const searchText = ref("");
const searchLoading = ref(false);
const searchResults = ref<TreeNode[]>([]);
const selectedPaths = ref<string[]>([]);
let scanStatusTimer: number | undefined;
let searchTimer: number | undefined;
let searchGeneration = 0;
let scanStartedAt = 0;
let activeScanPath = "";
let scanEntriesScanned = 0;

const updateScanStatus = () => {
  eventBus.emit("scan-progress", {
    path: activeScanPath,
    entriesScanned: scanEntriesScanned,
    elapsedMs: Date.now() - scanStartedAt
  });
};

const startScanStatus = (path: string) => {
  if (scanStatusTimer !== undefined) window.clearInterval(scanStatusTimer);
  activeScanPath = path;
  scanEntriesScanned = 0;
  scanStartedAt = Date.now();
  eventBus.emit("state-message", {text: "正在扫描"});
  updateScanStatus();
  scanStatusTimer = window.setInterval(updateScanStatus, 1000);
};

const stopScanStatus = () => {
  if (scanStatusTimer !== undefined) window.clearInterval(scanStatusTimer);
  scanStatusTimer = undefined;
};

const togglePath = (path: string, checked: boolean) => {
  selectedPaths.value = checked
    ? [...new Set([...selectedPaths.value, path])]
    : selectedPaths.value.filter(item => item !== path);
};

const renderPrefix = ({option}: {option: TreeOption}) => {
  const node = option as TreeNode;
  if (node.isLoadMore) return h(NIcon, {class: "tree-kind-icon"}, {default: () => h(AddOutline)});
  const icon = !node.readable ? LockClosedOutline : node.type === "directory" ? FolderOutline : DocumentTextOutline;
  return h(NIcon, {class: ["tree-kind-icon", !node.readable && "tree-kind-icon-muted"]}, {default: () => h(icon)});
};

const renderSuffix = ({option}: {option: TreeOption}) => {
  const node = option as TreeNode;
  if (node.isLoadMore) return null;
  return h("div", {class: "tree-node-suffix"}, [
    h("span", {class: ["tree-node-size", !node.complete && "tree-node-size-incomplete"]}, node.sizeLabel),
    h(NCheckbox, {
      checked: selectedPaths.value.includes(node.key),
      "aria-label": `选择 ${node.label}`,
      onUpdateChecked: (checked: boolean) => togglePath(node.key, checked),
      onClick: (event: MouseEvent) => event.stopPropagation()
    })
  ]);
};

const convertToTree = (node: FileSizeEntry): TreeNode => {
  const sizeLabel = !node.readable
    ? "不可读取"
    : `${node.complete ? "" : "部分 "}${MyTool.formatSize(node.size)}`;
  return {
    label: node.name,
    key: node.path,
    isLeaf: !node.has_children,
    type: node.is_dir ? "directory" : "file",
    size: node.size,
    sizeLabel,
    searchable: `${node.name} ${node.path}`.toLocaleLowerCase(),
    hasChildren: node.has_children,
    loaded: false,
    readable: node.readable,
    complete: node.complete,
    children: undefined
  };
};

const processTreeData = (payload: unknown) => {
  if (typeof payload !== "string") throw new Error("扫描结果格式无效");
  const result = JSON.parse(payload) as ScanSummary;
  const root = convertToTree(result.root);
  treeData.value = [root];
  selectedPaths.value = [];
  selectedKeys.value = [root.key];
  itemCount.value = result.item_count;
  incompleteCount.value = result.incomplete_count;
  scanReady.value = true;
};

const createLoadMoreNode = (parentPath: string, offset: number, total: number): TreeNode => ({
  label: `加载更多（${Math.min(offset + 1000, total).toLocaleString()} / ${total.toLocaleString()}）`,
  key: `more:${parentPath}:${offset}`,
  type: "file",
  readable: true,
  complete: true,
  size: 0,
  sizeLabel: "",
  searchable: "",
  hasChildren: false,
  loaded: true,
  isLoadMore: true,
  parentPath,
  nextOffset: offset,
  totalChildren: total,
  isLeaf: true,
  disabled: true
});

const findTreeNode = (path: string): TreeNode | undefined => {
  const pending = [...treeData.value];
  while (pending.length) {
    const node = pending.pop()!;
    if (node.key === path) return node;
    pending.push(...(node.children ?? []));
  }
  return undefined;
};

const loadChildrenPage = async (parent: TreeNode, offset: number, append: boolean) => {
  const page = await invoke<ChildrenPage>("file_size_children", {
    path: parent.key,
    offset,
    limit: 1000
  });
  const entries = page.entries.map(convertToTree);
  if (page.has_more) entries.push(createLoadMoreNode(parent.key, offset + page.entries.length, page.total));
  if (append) {
    const children = parent.children ?? [];
    const loadMoreIndex = children.findIndex(child => child.isLoadMore);
    if (loadMoreIndex >= 0) children.splice(loadMoreIndex, 1, ...entries);
    else children.push(...entries);
    parent.children = children;
  } else {
    parent.children = entries;
  }
};

const loadTreeNode = async (option: TreeOption) => {
  const node = option as TreeNode;
  if (node.loaded || !node.hasChildren) return;
  try {
    await loadChildrenPage(node, 0, false);
    node.loaded = true;
  } catch (error) {
    message.error(`读取目录失败：${String(error)}`);
  }
};

const loadMoreChildren = async (node: TreeNode) => {
  if (!node.parentPath || node.nextOffset === undefined || node.loadingMore) return;
  const parent = findTreeNode(node.parentPath);
  if (!parent) return;
  node.loadingMore = true;
  node.label = "正在加载…";
  try {
    await loadChildrenPage(parent, node.nextOffset, true);
  } catch (error) {
    node.loadingMore = false;
    node.label = `加载失败，点击重试（${node.totalChildren?.toLocaleString() ?? ""}）`;
    message.error(`读取目录失败：${String(error)}`);
  }
};

watch([searchText, treeLoading, scanReady], ([text, loading, ready]) => {
  searchGeneration += 1;
  const generation = searchGeneration;
  if (searchTimer !== undefined) window.clearTimeout(searchTimer);
  searchResults.value = [];
  const query = text.trim();
  if (!query || loading || !ready) {
    searchLoading.value = false;
    return;
  }

  searchLoading.value = true;
  searchTimer = window.setTimeout(async () => {
    try {
      const results = await invoke<Array<Omit<FileSizeEntry, "is_dir" | "has_children">>>("file_search", {
        query,
        limit: 1000
      });
      if (generation === searchGeneration) {
        searchResults.value = results.map(result => convertToTree({
          ...result,
          is_dir: false,
          has_children: false
        }));
      }
    } catch (error) {
      if (generation === searchGeneration) message.error(`搜索失败：${String(error)}`);
    } finally {
      if (generation === searchGeneration) searchLoading.value = false;
    }
  }, 250);
});

listen("file_size_result", (event) => {
  stopScanStatus();
  try {
    processTreeData(event.payload);
    eventBus.emit("state-message", {
      text: incompleteCount.value
        ? `扫描完成：${itemCount.value} 项，${incompleteCount.value} 项未能完整读取`
        : `扫描完成：${itemCount.value} 项`
    });
  } catch (error) {
    treeData.value = [];
    scanReady.value = false;
    message.error(`处理扫描结果失败：${String(error)}`);
    eventBus.emit("state-message", {text: "处理扫描结果失败"});
  } finally {
    treeLoading.value = false;
  }
});

listen("file_size_result_error", (event) => {
  stopScanStatus();
  treeLoading.value = false;
  scanReady.value = false;
  eventBus.emit("state-message", {text: `扫描失败：${String(event.payload)}`});
});

listen<ScanProgressEvent>("file_size_progress", (event) => {
  activeScanPath = event.payload.path;
  scanEntriesScanned = event.payload.entries_scanned;
  updateScanStatus();
});

listen("file_deleted", () => {
  treeLoading.value = false;
  message.success("删除成功");
  eventBus.emit("state-message", {text: `已删除：${currentSelectPath.value}`});
  selectPath(currentPath.value);
});

listen("file_deleted_error", (event) => {
  treeLoading.value = false;
  message.error(String(event.payload));
});

const selectPath = (path: string) => {
  if (!path) return;
  currentPath.value = path;
  treeData.value = [];
  itemCount.value = 0;
  incompleteCount.value = 0;
  searchResults.value = [];
  scanReady.value = false;
  selectedPaths.value = [];
  treeLoading.value = true;
  startScanStatus(path);
  invoke("file_size", {path}).catch((error) => {
    stopScanStatus();
    treeLoading.value = false;
    message.error(`启动扫描失败：${String(error)}`);
    eventBus.emit("state-message", {text: "启动扫描失败"});
  });
};

const nodeProps = ({option}: {option: TreeOption}) => ({
  onClick() {
    const node = option as TreeNode;
    if (node.isLoadMore) return;
    currentSelectPath.value = node.key;
    selectedKeys.value = [node.key];
  },
  onContextmenu(event: MouseEvent) {
    event.preventDefault();
    const node = option as TreeNode;
    if (node.isLoadMore) return;
    currentSelectPath.value = node.key;
    selectedKeys.value = [node.key];
    treeNodeContextMenuRef.value?.showContextMenu(event);
  }
});

const fileDeleteClick = () => {
  dialog.warning({
    title: "确认删除",
    content: `确认删除此项及其内容？\n${currentSelectPath.value}`,
    positiveText: "删除",
    negativeText: "取消",
    onPositiveClick: () => {
      treeLoading.value = true;
      eventBus.emit("state-message", {text: `正在删除：${currentSelectPath.value}`});
      invoke("file_delete", {path: currentSelectPath.value}).catch((error) => {
        treeLoading.value = false;
        message.error(String(error));
      });
    }
  });
};

const showBatchResult = (result: {succeeded: string[]; failed: Array<{path: string; error: string}>}) => {
  selectedPaths.value = [];
  treeLoading.value = false;
  const summary = `${result.succeeded.length} 项成功，${result.failed.length} 项失败`;
  if (result.failed.length) {
    message.warning(`${summary}：${result.failed[0].error}`);
  } else {
    message.success(summary);
  }
  eventBus.emit("state-message", {text: summary});
  selectPath(currentPath.value);
};

const deleteSelected = () => {
  if (!selectedPaths.value.length) return;
  const paths = [...selectedPaths.value];
  dialog.warning({
    title: "确认批量删除",
    content: `将永久删除 ${paths.length} 个项目及所选目录中的全部内容，是否继续？`,
    positiveText: "删除",
    negativeText: "取消",
    onPositiveClick: async () => {
      treeLoading.value = true;
      try {
        const result = await invoke<{succeeded: string[]; failed: Array<{path: string; error: string}>}>("file_delete_many", {paths});
        showBatchResult(result);
      } catch (error) {
        treeLoading.value = false;
        message.error(String(error));
      }
    }
  });
};

const moveSelected = async () => {
  if (!selectedPaths.value.length) return;
  try {
    const destination = await open({multiple: false, directory: true, title: "选择移动目标目录"});
    if (typeof destination !== "string" || !destination) return;
    treeLoading.value = true;
    const result = await invoke<{succeeded: string[]; failed: Array<{path: string; error: string}>}>("file_move_many", {
      paths: [...selectedPaths.value],
      destination
    });
    showBatchResult(result);
  } catch (error) {
    treeLoading.value = false;
    message.error(String(error));
  }
};

const revealPath = async (path: string) => {
  try {
    await invoke("reveal_path", {path});
  } catch (error) {
    message.error(String(error));
  }
};

const selectMenu = (item: string) => {
  if (item === "refresh") selectPath(currentPath.value);
  if (item === "delete") fileDeleteClick();
  if (item === "reveal") revealPath(currentSelectPath.value);
  if (item === "open") {
    invoke("open_folder", {path: currentSelectPath.value}).catch((error) => message.error(String(error)));
  }
};

const isEmpty = computed(() => treeData.value.length === 0);

defineExpose({selectPath});
</script>

<template>
  <div class="tree-region">
    <div class="tree-actions">
      <n-input v-model:value="searchText" clearable placeholder="搜索文件名或路径" size="small">
        <template #prefix><n-icon><SearchOutline /></n-icon></template>
      </n-input>
      <span class="selection-count">已选 {{ selectedPaths.length }} 项</span>
      <n-button size="small" secondary :disabled="!selectedPaths.length" @click="moveSelected">移动</n-button>
      <n-button size="small" type="error" secondary :disabled="!selectedPaths.length" @click="deleteSelected">删除</n-button>
    </div>
    <div v-if="searchText.trim()" class="search-results">
      <div class="search-columns">
        <span>文件与位置</span>
        <span>大小</span>
      </div>
      <n-scrollbar trigger="none">
        <n-empty v-if="!searchResults.length" :description="searchLoading ? '正在搜索…' : '没有匹配的文件'" size="small" />
        <div v-for="node in searchResults" :key="node.key" class="search-result-row">
          <n-checkbox
            :checked="selectedPaths.includes(node.key)"
            :aria-label="`选择 ${node.label}`"
            @update:checked="checked => togglePath(node.key, checked)"
          />
          <div class="search-result-copy" :title="node.key">
            <strong>{{ node.label }}</strong>
            <span>{{ node.key }}</span>
          </div>
          <span class="tree-node-size">{{ node.sizeLabel }}</span>
          <n-button quaternary circle size="small" title="在文件管理器中定位" aria-label="在文件管理器中定位" @click="revealPath(node.key)">
            <template #icon><n-icon><FolderOpenOutline /></n-icon></template>
          </n-button>
        </div>
      </n-scrollbar>
      <small v-if="searchResults.length === 1000" class="search-limit">仅显示前 1000 个结果</small>
    </div>
    <div v-else class="tree-column-head">
      <span>名称</span>
      <span>大小</span>
    </div>
    <n-spin v-show="!searchText.trim()" :show="treeLoading" class="tree-spin">
      <n-scrollbar class="tree-scrollbar" trigger="none">
        <n-tree
          :data="treeData"
          :selected-keys="selectedKeys"
          :on-load="loadTreeNode"
          :node-props="nodeProps"
          :render-prefix="renderPrefix"
          :render-suffix="renderSuffix"
          virtual-scroll
          block-line
          class="directory-tree"
        >
          <template #label="{option}">
            <button
              v-if="(option as TreeNode).isLoadMore"
              type="button"
              class="tree-load-more"
              :disabled="(option as TreeNode).loadingMore"
              @click.stop="loadMoreChildren(option as TreeNode)"
            >{{ option.label }}</button>
            <span v-else class="tree-node-name" :class="{'tree-node-unreadable': !(option as TreeNode).readable}">{{ option.label }}</span>
          </template>
          <template #empty>
            <div class="empty-state">
              <div class="empty-icon"><n-icon><FolderOpenOutline /></n-icon></div>
              <strong>{{ treeLoading ? "正在读取目录" : "没有目录内容" }}</strong>
              <span>{{ treeLoading ? "大型目录可能需要一些时间" : "选择目录后，内容会显示在这里" }}</span>
            </div>
          </template>
        </n-tree>
      </n-scrollbar>
    </n-spin>
    <div v-if="!searchText.trim() && !isEmpty && incompleteCount" class="partial-notice">
      <n-icon><LockClosedOutline /></n-icon>
      <span>{{ incompleteCount }} 项无法完整读取，显示的目录大小可能低于实际值</span>
    </div>
    <TreeNodeContextMenu ref="treeNodeContextMenuRef" @select="selectMenu" />
  </div>
</template>
