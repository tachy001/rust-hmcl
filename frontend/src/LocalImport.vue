<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { api, type Job } from './api';
import Icon from './Icon.vue';
const props = defineProps<{
  id: string;
  content: 'mods' | 'resources';
  busy: boolean;
  submit: (job: Job) => Promise<boolean>;
}>();
const selected = ref<string[]>([]);
const error = ref('');
const picking = ref(false);
const pending = ref(false);
const dragging = ref(false);
const dropReady = ref(false);
const host = ref<HTMLElement | null>(null);
const submitted = ref(false);
let alive = true;
let unlisten: (() => void) | undefined;
const name = (path: string) => path.replaceAll('\\', '/').split('/').pop() ?? path;
function choose(paths: string[]) {
  if (props.busy || pending.value) return;
  submitted.value = false;
  if (!paths.length || paths.length > 256) {
    error.value = '每次请选择 1–256 个文件或文件夹。';
    return;
  }
  selected.value = [...new Set(paths)];
  error.value = '';
}
async function pick(folder = false) {
  if (picking.value || props.busy || pending.value) return;
  picking.value = true;
  try {
    const paths = await api.pickImport(props.content, folder);
    if (alive && paths.length) choose(paths);
  } catch (cause) {
    if (alive) error.value = String(cause);
  } finally {
    if (alive) picking.value = false;
  }
}
async function importSelection() {
  if (!selected.value.length || props.busy || pending.value) return;
  pending.value = true;
  try {
    const accepted = await props.submit({
      kind: 'import_files',
      id: props.id,
      content: props.content,
      paths: [...selected.value],
    });
    if (alive && accepted) {
      selected.value = [];
      submitted.value = true;
    }
  } finally {
    if (alive) pending.value = false;
  }
}
onMounted(async () => {
  try {
    const stop = await getCurrentWebview().onDragDropEvent((event) => {
      if (!alive) return;
      if (host.value?.closest('[inert]')) {
        dragging.value = false;
        return;
      }
      const payload = event.payload;
      dragging.value = !props.busy && (payload.type === 'enter' || payload.type === 'over');
      if (payload.type === 'drop' && !props.busy) choose(payload.paths);
    });
    if (alive) {
      unlisten = stop;
      dropReady.value = true;
    } else stop();
  } catch {
    /* File selection remains available when native drag events are unavailable. */
  }
});
onUnmounted(() => {
  alive = false;
  unlisten?.();
});
</script>
<template>
  <section
    ref="host"
    class="local-import"
    :class="{ dragging: dragging && !busy }"
    :aria-label="content === 'mods' ? '导入本地模组' : '导入本地资源包'"
    @dragover.prevent
    @drop.prevent
  >
    <div class="import-heading">
      <span class="import-symbol"><Icon name="download" /></span>
      <div>
        <b>{{
          dragging
            ? '松开以选择文件'
            : content === 'mods'
              ? '把喜欢的模组带进来'
              : '为世界换一副模样'
        }}</b>
        <p>
          {{
            content === 'mods'
              ? `${dropReady ? '选择或拖入' : '选择本地'} .jar 文件，源文件会保留。`
              : `${dropReady ? '选择或拖入' : '选择本地'} ZIP／资源包文件夹，导入后可手动启用。`
          }}
        </p>
      </div>
    </div>
    <div class="import-actions">
      <button :disabled="busy || picking || pending" @click="pick()">
        <Icon name="add" />{{ picking ? '选择中…' : '选择文件' }}</button
      ><button
        v-if="content === 'resources'"
        :disabled="busy || picking || pending"
        @click="pick(true)"
      >
        <Icon name="folder" />选择文件夹
      </button>
    </div>
    <div v-if="selected.length" class="import-selection">
      <ul aria-label="待导入文件">
        <li v-for="path in selected" :key="path" :title="path">
          <Icon :name="content === 'mods' ? 'extension' : 'texture'" /><span>{{ name(path) }}</span
          ><button
            class="icon-button"
            :aria-label="`移除 ${name(path)}`"
            :disabled="busy || pending"
            @click="selected = selected.filter((item) => item !== path)"
          >
            ×
          </button>
        </li>
      </ul>
      <div class="import-selection-actions">
        <span>同名内容不会覆盖</span
        ><button :disabled="busy || pending" @click="selected = []">清空</button
        ><button class="primary" :disabled="busy || pending" @click="importSelection">
          {{ pending ? '提交中…' : `导入 ${selected.length} 项` }}
        </button>
      </div>
    </div>
    <p v-if="error" role="alert" class="import-error">{{ error }}</p>
    <p v-if="submitted" role="status" class="import-result">
      已提交导入，进度和结果显示在窗口底部。
    </p>
  </section>
</template>
