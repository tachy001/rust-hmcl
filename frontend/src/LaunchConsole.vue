<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import Icon from './Icon.vue';
import { api, type Task } from './api';
const props = defineProps<{ task: Task; recoverable: boolean }>();
defineEmits<{ recover: [action: string] }>();
const query = ref('');
const level = ref('all');
const following = ref(true);
const viewport = ref<HTMLElement | null>(null);
const exporting = ref(false);
const exportStatus = ref('');
const exportError = ref('');
function severity(text: string) {
  if (/\b(?:ERROR|FATAL|SEVERE)\b|(?:^|:\s)(?:[\w$]+\.)*\w*(?:Exception|Error)(?=:|$)/i.test(text))
    return 'error';
  if (/\b(?:WARN|WARNING)\b/.test(text)) return 'warn';
  return 'info';
}
const rows = computed(() =>
  props.task.logs.map((text, index) => ({ text, index, level: severity(text) })),
);
const visible = computed(() =>
  rows.value.filter(
    (row) =>
      (level.value === 'all' || row.level === level.value) &&
      row.text.toLowerCase().includes(query.value.toLowerCase()),
  ),
);
const counts = computed(() => ({
  error: rows.value.filter((row) => row.level === 'error').length,
  warn: rows.value.filter((row) => row.level === 'warn').length,
}));
async function bottom() {
  await nextTick();
  if (following.value && viewport.value) viewport.value.scrollTop = viewport.value.scrollHeight;
}
function scrolled() {
  const el = viewport.value;
  if (el && el.scrollHeight - el.scrollTop - el.clientHeight > 24) following.value = false;
}
function follow() {
  following.value = true;
  void bottom();
}
watch(
  () => [props.task.id, props.task.logs.length, props.task.logs.at(-1)],
  () => void bottom(),
  { flush: 'post' },
);
watch(
  () => props.task.id,
  () => {
    exportStatus.value = '';
    exportError.value = '';
  },
);
watch([query, level], () => {
  if (following.value) void bottom();
});
onMounted(() => void bottom());
async function exportReport() {
  const requestedTask = props.task.id;
  exporting.value = true;
  exportStatus.value = '';
  exportError.value = '';
  try {
    const path = await api.exportTaskReport(requestedTask);
    if (props.task.id === requestedTask)
      exportStatus.value = path ? `已保存：${path}` : '已取消保存';
  } catch (error) {
    if (props.task.id === requestedTask) exportError.value = String(error);
  } finally {
    exporting.value = false;
  }
}
const actionNames: Record<string, string> = {
  java: '打开 Java 管理',
  settings: '检查实例设置',
  repair: '校验游戏文件',
  mods: '检查实例模组',
};
</script>
<template>
  <div class="launch-console">
    <div class="toolbar">
      <div>
        <p class="eyebrow">ASTER / 旅程信号台</p>
        <h1>任务与游戏日志</h1>
      </div>
      <button :disabled="!task.id || exporting" @click="exportReport">
        <Icon name="download" />{{ exporting ? '保存中…' : '导出任务报告' }}
      </button>
    </div>
    <div class="console-summary" :class="{ failed: task.error }">
      <span class="console-signal" :class="{ live: task.running }"
        ><Icon :name="task.error ? 'error' : task.running ? 'rocket_launch' : 'check'"
      /></span>
      <div>
        <b>{{ task.status || '等待下一段旅程' }}</b>
        <p>
          {{
            task.launch_id
              ? `启动实例 · ${task.launch_id}`
              : task.id
                ? `任务 #${task.id}`
                : '启动游戏后，可以在这里查看实时输出。'
          }}
        </p>
      </div>
      <span class="console-state">{{
        task.game_running ? '游戏运行中' : task.running ? '任务进行中' : '当前任务已停止'
      }}</span>
    </div>
    <section v-if="task.error" class="launch-diagnosis" aria-label="任务错误与启动线索">
      <div class="section-heading">
        <h2>{{ task.launch_id ? '启动失败后的下一步' : '任务错误' }}</h2>
        <span class="java-label">保留原始错误</span>
        <button
          v-if="task.launch_id && recoverable"
          :disabled="task.running"
          @click="$emit('recover', 'settings')"
        >
          实例设置
        </button>
      </div>
      <p class="console-error">{{ task.error }}</p>
      <p v-if="task.launch_id && !task.clues.length" class="diagnosis-note">
        暂未识别出明确线索。退出码本身不足以判断原因，请查看输出或导出报告继续排查。
      </p>
      <article v-for="clue in task.clues" :key="clue.title" class="diagnosis-clue">
        <Icon name="info" />
        <div>
          <h3>{{ clue.title }}</h3>
          <code>{{ clue.evidence }}</code>
          <p>{{ clue.suggestion }}</p>
        </div>
        <button
          :disabled="task.running || (clue.action !== 'java' && !recoverable)"
          @click="$emit('recover', clue.action)"
        >
          {{ actionNames[clue.action] ?? '检查实例设置' }}<Icon name="arrow_forward" />
        </button>
      </article>
      <p v-if="task.clues.length" class="diagnosis-note">
        这些是根据错误与保留输出匹配的排查线索，不等同于确定的故障原因。
      </p>
    </section>
    <p v-if="exportStatus" class="console-export-status" role="status">{{ exportStatus }}</p>
    <p v-if="exportError" class="console-error" role="alert">{{ exportError }}</p>
    <div class="console-tools">
      <label class="console-search"
        ><Icon name="search" /><input
          v-model="query"
          aria-label="搜索日志"
          placeholder="搜索当前保留的输出"
          type="search"
      /></label>
      <select v-model="level" aria-label="日志级别">
        <option value="all">全部输出</option>
        <option value="error">错误</option>
        <option value="warn">警告</option>
        <option value="info">其他输出</option>
      </select>
      <button
        class="console-follow"
        :aria-pressed="following"
        @click="following ? (following = false) : follow()"
      >
        <Icon name="arrow_drop_down" />{{ following ? '跟随输出' : '继续跟随' }}
      </button>
    </div>
    <div class="console-counts">
      <span>显示 {{ visible.length }} / {{ task.logs.length }} 行</span
      ><span>错误 {{ counts.error }} · 警告 {{ counts.warn }}</span>
    </div>
    <div
      ref="viewport"
      class="console-output"
      tabindex="0"
      aria-label="游戏输出"
      @scroll="scrolled"
    >
      <div
        v-for="row in visible"
        :key="`${task.id}/${row.index}`"
        class="console-line"
        :class="row.level"
      >
        <span aria-hidden="true">{{ row.index + 1 }}</span
        ><code>{{ row.text || ' ' }}</code>
      </div>
      <p v-if="!visible.length" class="console-empty">
        {{
          task.logs.length
            ? '没有匹配的输出，试试其他关键词或级别。'
            : '暂无游戏输出。启动前检查失败时，可能还没有创建游戏进程。'
        }}
      </p>
    </div>
    <p class="console-footnote">
      当前任务最多保留 500 行输出。筛选只影响显示；导出报告包含当前保留的全部输出与错误。
    </p>
  </div>
</template>
