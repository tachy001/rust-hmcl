<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { api, type Instance, type Job, type ModpackPreview, type Task } from './api';
import Icon from './Icon.vue';
const props = defineProps<{
  task: Task;
  instances: Instance[];
  submit: (job: Job) => Promise<boolean>;
}>();
const emit = defineEmits<{ library: [] }>();
const preview = ref<ModpackPreview | null>(null);
const path = ref('');
const id = ref('');
const optional = ref(false);
const error = ref('');
const checking = ref(false);
const picking = ref(false);
const submitting = ref(false);
const submitted = ref('');
const dragging = ref(false);
const dropReady = ref(false);
const host = ref<HTMLElement | null>(null);
const completePanel = ref<HTMLElement | null>(null);
let alive = true,
  request = 0;
let stopDrop: (() => void) | undefined;
const filename = computed(() => path.value.replaceAll('\\', '/').split('/').pop());
const ready = computed(
  () =>
    !!submitted.value &&
    submitted.value === id.value.trim() &&
    !props.task.running &&
    props.instances.some((instance) => instance.id === submitted.value),
);
watch(ready, async (value) => {
  if (!value) return;
  await nextTick();
  if (alive) completePanel.value?.scrollIntoView({ block: 'nearest', behavior: 'auto' });
});
const nameError = computed(() => {
  const name = id.value.trim();
  if (!name) return '请输入新实例名称';
  if (
    name.length > 100 ||
    /[<>:"/\\|?*\u0000-\u001f\u007f]/.test(name) ||
    /[. ]$/.test(name) ||
    name === '.' ||
    name === '..'
  )
    return '名称不能包含路径符号或以点结尾';
  if (/^(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(\.|$)/i.test(name)) return '此名称是系统保留名称';
  if (name === preview.value?.minecraft) return '请使用与基础游戏版本不同的名称';
  return '';
});
async function inspect(selected: string) {
  if (props.task.running || submitting.value) return;
  const token = ++request;
  checking.value = true;
  error.value = '';
  preview.value = null;
  submitted.value = '';
  try {
    const result = await api.inspectModpack(selected);
    if (alive && token === request) {
      preview.value = result;
      path.value = selected;
      id.value = result.suggested_name;
      optional.value = false;
    }
  } catch (cause) {
    if (alive && token === request) error.value = String(cause);
  } finally {
    if (alive && token === request) checking.value = false;
  }
}
async function pick() {
  if (picking.value || props.task.running || submitting.value) return;
  picking.value = true;
  try {
    const selected = await api.pickModpack();
    if (alive && selected) await inspect(selected);
  } catch (cause) {
    if (alive) error.value = String(cause);
  } finally {
    if (alive) picking.value = false;
  }
}
async function install() {
  if (!preview.value || nameError.value || props.task.running || submitting.value) return;
  submitting.value = true;
  const name = id.value.trim();
  try {
    if (
      (await props.submit({
        kind: 'import_pack',
        path: path.value,
        id: name,
        optional: optional.value,
        source_sha1: preview.value.source_sha1,
      })) &&
      alive
    )
      submitted.value = name;
  } finally {
    if (alive) submitting.value = false;
  }
}
onMounted(async () => {
  try {
    const stop = await getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (!alive || host.value?.closest('[inert]')) return;
      dragging.value = !props.task.running && (payload.type === 'enter' || payload.type === 'over');
      if (payload.type === 'drop' && !props.task.running) {
        if (payload.paths.length !== 1) {
          error.value = '每次请选择一个 .mrpack 文件';
          return;
        }
        void inspect(payload.paths[0]);
      }
    });
    if (alive) {
      stopDrop = stop;
      dropReady.value = true;
    } else stop();
  } catch {
    /* The file picker remains available without native drop support. */
  }
});
onUnmounted(() => {
  alive = false;
  request++;
  stopDrop?.();
});
</script>
<template>
  <div ref="host" class="pack-installer">
    <div class="toolbar">
      <div>
        <p class="eyebrow">一个文件，开启另一种世界</p>
        <h1>导入整合包</h1>
      </div>
      <button @click="emit('library')"><Icon name="folder" />游戏库</button>
    </div>
    <section
      class="pack-drop-zone"
      :class="{ dragging: dragging && !task.running }"
      @dragover.prevent
      @drop.prevent
    >
      <span class="pack-source-icon"><Icon name="deployed_code" /></span>
      <div>
        <h2>{{ dragging ? '松开以读取整合包' : '带上你的冒险行囊' }}</h2>
        <p>
          {{ dropReady ? '选择或拖入 .mrpack 文件' : '选择本地 .mrpack 文件' }}，先预览，再安装。
        </p>
        <small>支持原版、Fabric 与 Quilt；Forge／NeoForge 暂不支持。</small>
      </div>
      <button
        class="primary"
        :disabled="task.running || picking || checking || submitting"
        @click="pick"
      >
        <Icon name="add" />{{ checking ? '读取中…' : picking ? '选择中…' : '选择整合包' }}
      </button>
    </section>
    <p v-if="error" role="alert" class="pack-error"><Icon name="info" />{{ error }}</p>
    <p v-if="checking" role="status" class="subtitle">正在校验索引与包内路径…</p>
    <section v-if="ready" ref="completePanel" class="pack-ready" role="status">
      <Icon name="check" />
      <div>
        <h2>{{ submitted }} 已准备好</h2>
        <p>到游戏库管理或启动这个实例。</p>
      </div>
      <button @click="emit('library')">打开游戏库<Icon name="arrow_forward" /></button>
    </section>
    <form v-if="preview" class="pack-preview" @submit.prevent="install">
      <div class="pack-preview-heading">
        <div>
          <span class="eyebrow">整合包预览</span>
          <h2>{{ preview.name || '未命名整合包' }}</h2>
          <p v-if="preview.summary">{{ preview.summary }}</p>
        </div>
        <span class="pack-version">{{ preview.version || '未提供版本号' }}</span>
      </div>
      <div class="pack-environment">
        <span><img src="/hmcl/grass.png" alt="" />Minecraft {{ preview.minecraft }}</span
        ><span
          ><Icon name="deployed_code" />{{
            preview.loader ? `${preview.loader} ${preview.loader_version}` : '原版'
          }}</span
        >
      </div>
      <div class="pack-file-counts">
        <div>
          <b>{{ preview.required }}</b
          ><span>必需客户端文件</span>
        </div>
        <div>
          <b>{{ preview.optional }}</b
          ><span>可选客户端文件</span>
        </div>
        <div>
          <b>{{ preview.server_only }}</b
          ><span>仅服务端，跳过</span>
        </div>
      </div>
      <fieldset :disabled="task.running || submitting">
        <label class="pack-name-label"
          >新实例名称<input
            v-model="id"
            type="text"
            maxlength="100"
            aria-label="整合包实例名称"
            :aria-invalid="!!nameError"
            required
        /></label>
        <p v-if="nameError" class="pack-name-error">{{ nameError }}</p>
        <label class="switch-row"
          ><span>安装可选客户端文件<small>必需文件始终安装，服务端专用文件始终跳过。</small></span
          ><input
            v-model="optional"
            role="switch"
            type="checkbox"
            :disabled="preview.optional === 0"
        /></label>
        <div class="pack-isolation">
          <Icon name="folder" />
          <p><b>独立游戏目录</b><span>这个整合包的模组、配置和存档会保存在新实例中。</span></p>
        </div>
        <div class="pack-install-actions">
          <span :title="path">{{ filename }}</span
          ><button class="primary" :disabled="!!nameError || checking || ready">
            <Icon name="download" />{{ task.running ? '正在执行任务…' : '安装到新实例' }}
          </button>
        </div>
      </fieldset>
    </form>
    <p class="pack-footnote">
      安装中断后可用同一个文件、同一个实例名继续；已有完整实例不会覆盖。所需 Java
      在启动游戏时检测或下载。
    </p>
  </div>
</template>
