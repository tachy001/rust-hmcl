<script setup lang="ts">
import { computed, onUnmounted, ref } from 'vue';
import { api, type Config, type Job, type Runtime, type Snapshot, type Task } from './api';
import Icon from './Icon.vue';
const props = defineProps<{
  config: Config;
  task: Task;
  save: (config: Config) => Promise<boolean>;
  submit: (job: Job) => Promise<boolean>;
}>();
const emit = defineEmits<{ snapshot: [snapshot: Snapshot] }>();
const detected = ref<Runtime[]>([]);
const manualPath = ref('');
const scanned = ref(false);
const working = ref('');
const error = ref('');
const major = ref(21);
const hiddenOpen = ref(false);
let alive = true;
const key = (path: string) =>
  /^[a-z]:[\\/]|^\\\\/i.test(path) ? path.replaceAll('\\', '/').toLowerCase() : path;
const same = (a: string, b: string) => key(a) === key(b);
const blocked = computed(() => props.task.running || !!working.value);
const rows = computed(() => {
  const values = new Map<string, { path: string; runtime: Runtime | null; registered: boolean }>();
  for (const runtime of detected.value)
    values.set(key(runtime.path), { path: runtime.path, runtime, registered: false });
  for (const path of [
    ...props.config.java_paths,
    ...(props.config.java ? [props.config.java] : []),
  ]) {
    const previous = values.get(key(path));
    values.set(key(path), {
      path: previous?.path ?? path,
      runtime: previous?.runtime ?? null,
      registered: props.config.java_paths.some((value) => same(value, path)),
    });
  }
  return [...values.values()]
    .filter((row) => !props.config.hidden_java.some((path) => same(row.path, path)))
    .sort(
      (a, b) => (b.runtime?.major ?? 0) - (a.runtime?.major ?? 0) || a.path.localeCompare(b.path),
    );
});
async function act(kind: string, action: () => Promise<void>) {
  if (blocked.value) return;
  working.value = kind;
  error.value = '';
  try {
    await action();
  } catch (cause) {
    if (alive) error.value = String(cause);
  } finally {
    if (alive) working.value = '';
  }
}
function remember(runtime: Runtime) {
  detected.value = [...detected.value.filter((value) => !same(value.path, runtime.path)), runtime];
}
async function register(path: string, choose = false) {
  const result = await api.registerJava(path.trim());
  if (!alive) return;
  remember(result.runtime);
  emit('snapshot', result.snapshot);
  manualPath.value = '';
  if (choose) await props.save({ ...result.snapshot.config, java: result.runtime.path });
}
async function scan() {
  await act('scan', async () => {
    const result = await api.runtimes();
    if (alive) {
      detected.value = result;
      scanned.value = true;
    }
  });
}
async function choose(row: { path: string; runtime: Runtime | null }) {
  await act('choose', async () => {
    if (row.runtime) await props.save({ ...props.config, java: row.path });
    else await register(row.path, true);
  });
}
async function hide(path: string) {
  await act('hide', async () => {
    await props.save({
      ...props.config,
      hidden_java: [...props.config.hidden_java, path],
      java: same(props.config.java, path) ? '' : props.config.java,
    });
  });
}
async function restore(path: string) {
  await act('restore', async () => {
    await props.save({
      ...props.config,
      hidden_java: props.config.hidden_java.filter((value) => !same(value, path)),
    });
  });
}
async function pick() {
  await act('picker', async () => {
    const path = await api.pickJava();
    if (alive && path) manualPath.value = path;
  });
}
onUnmounted(() => {
  alive = false;
});
</script>
<template>
  <div class="java-manager">
    <div class="toolbar">
      <div>
        <p class="eyebrow">给每一种世界，合适的运行环境</p>
        <h1>Java 管理</h1>
      </div>
      <button :disabled="blocked" @click="scan">
        <Icon name="refresh" :class="{ spinning: working === 'scan' }" />{{
          working === 'scan' ? '检测中…' : '检测本机'
        }}
      </button>
    </div>
    <p class="subtitle">启动器本身无需 Java；Minecraft 使用独立的运行环境。</p>
    <button
      class="java-auto-card"
      :aria-pressed="!config.java"
      :class="{ selected: !config.java }"
      :disabled="blocked"
      @click="
        act('automatic', async () => {
          await save({ ...config, java: '' });
        })
      "
    >
      <span class="java-auto-icon"><Icon name="deployed_code" /></span
      ><span
        ><b>按游戏自动选择 Java</b
        ><small>{{
          config.java
            ? '当前使用手动指定的全局默认环境'
            : '根据游戏所需版本选择，实例单独设置仍会优先使用'
        }}</small></span
      ><Icon v-if="!config.java" name="check" />
    </button>
    <p v-if="config.java" class="java-current-path">
      <span>全局默认</span><code>{{ config.java }}</code>
    </p>
    <p v-if="error" class="java-error" role="alert"><Icon name="info" />{{ error }}</p>
    <section class="java-add-card">
      <div class="section-heading">
        <h2>添加已有 Java</h2>
        <span class="java-label">本机文件</span>
      </div>
      <form @submit.prevent="act('add', () => register(manualPath))">
        <label class="java-path-label"
          >Java 可执行文件<input
            v-model="manualPath"
            type="text"
            aria-label="Java 可执行文件路径"
            placeholder="选择或填写 bin 目录中的 java 可执行文件"
            :disabled="blocked"
            required
        /></label>
        <div class="java-add-actions">
          <span>添加时读取版本信息，确认可用后保存路径。</span
          ><button type="button" :disabled="blocked" @click="pick">
            <Icon name="folder" />选择文件</button
          ><button class="primary" :disabled="blocked || !manualPath.trim()">
            {{ working === 'add' ? '检测中…' : '添加并检测' }}
          </button>
        </div>
      </form>
    </section>
    <div class="section-heading java-list-heading">
      <h2>
        可选运行环境<span>{{ rows.length }}</span>
      </h2>
      <span class="java-label">{{ scanned ? '已检测本机' : '点击检测本机以发现更多环境' }}</span>
    </div>
    <div class="java-runtime-list">
      <article
        v-for="row in rows"
        :key="row.path"
        class="java-runtime"
        :class="{ selected: same(config.java, row.path) }"
      >
        <span class="java-runtime-icon"><Icon name="deployed_code" /></span>
        <div class="java-runtime-copy">
          <h3>
            {{
              row.runtime
                ? `Java ${row.runtime.major} · ${row.runtime.vendor}`
                : '已登记的 Java 路径'
            }}<span v-if="same(config.java, row.path)" class="java-selected-label">全局默认</span>
          </h3>
          <p v-if="row.runtime">
            {{ row.runtime.version }} · {{ row.runtime.architecture }} ·
            {{ row.runtime.is_jdk ? 'JDK' : '运行环境' }}
          </p>
          <p v-else>检测后可设为全局默认环境</p>
          <p v-if="row.runtime && !row.runtime.compatible" class="java-arch-warning">
            与当前启动器架构不匹配，请选择其他环境。
          </p>
          <code>{{ row.path }}</code>
        </div>
        <div class="java-runtime-actions">
          <button
            :disabled="blocked || same(config.java, row.path) || row.runtime?.compatible === false"
            :aria-label="`选择 Java ${row.path}`"
            @click="choose(row)"
          >
            {{ row.runtime ? '设为默认' : '检测并选择' }}</button
          ><button
            class="text-button"
            :disabled="blocked"
            :aria-label="`隐藏 Java ${row.path}`"
            @click="hide(row.path)"
          >
            隐藏
          </button>
        </div>
      </article>
    </div>
    <div v-if="!rows.length" class="java-empty">
      <Icon name="deployed_code" />
      <h2>{{ scanned ? '还没有找到可用环境' : '让启动器认识你的 Java' }}</h2>
      <p>
        {{
          scanned
            ? '可以手动添加 Java，或下载匹配的运行环境。'
            : '检测本机、填写已有路径，或下载一个新的运行环境。'
        }}
      </p>
    </div>
    <section v-if="config.hidden_java.length" class="java-hidden">
      <button
        class="java-hidden-toggle"
        :aria-expanded="hiddenOpen"
        @click="hiddenOpen = !hiddenOpen"
      >
        <Icon name="arrow_drop_down" :class="{ rotated: hiddenOpen }" />已隐藏
        {{ config.hidden_java.length }} 个环境
      </button>
      <div v-if="hiddenOpen">
        <p>隐藏只影响候选列表和自动选择，文件及实例单独设置会保留。</p>
        <div v-for="path in config.hidden_java" :key="path" class="java-hidden-row">
          <code>{{ path }}</code
          ><button :disabled="blocked" :aria-label="`恢复 Java ${path}`" @click="restore(path)">
            恢复
          </button>
        </div>
      </div>
    </section>
    <section class="java-download">
      <div class="java-download-copy">
        <span class="eyebrow">还没有合适的环境？</span>
        <h2>下载 Temurin Java</h2>
        <p>优先使用运行环境包；当前平台没有 JRE 时使用 JDK。</p>
        <label class="switch-row"
          ><input
            type="checkbox"
            :checked="config.auto_java"
            :disabled="blocked"
            @change="
              act('autodownload', async () => {
                await save({ ...config, auto_java: !config.auto_java });
              })
            "
          /><span>游戏缺少 Java 时自动下载</span></label
        >
      </div>
      <div class="java-download-actions">
        <label
          >Java 版本<select v-model.number="major" aria-label="Java 版本" :disabled="blocked">
            <option v-for="value in [8, 11, 17, 21, 25]" :key="value" :value="value">
              Java {{ value }}
            </option>
          </select></label
        ><button
          class="primary"
          :disabled="blocked"
          @click="
            act('download', async () => {
              await submit({ kind: 'java', major });
            })
          "
        >
          <Icon name="download" />下载运行环境
        </button>
      </div>
    </section>
  </div>
</template>
