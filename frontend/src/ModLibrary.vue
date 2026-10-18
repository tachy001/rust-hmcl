<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import Icon from './Icon.vue';
import LocalImport from './LocalImport.vue';
import { api, type Job, type Mod, type ModEnvironment, type ModProject } from './api';
const props = defineProps<{
  id: string;
  mods: Mod[];
  busy: boolean;
  loading: boolean;
  submit: (job: Job) => Promise<boolean>;
}>();
const emit = defineEmits<{
  refresh: [];
  toggle: [name: string];
  start: [job: Job];
  components: [];
}>();
const view = ref<'local' | 'discover'>('local');
const filter = ref('');
const query = ref('');
const environment = ref<ModEnvironment | null>(null);
const environmentError = ref('');
const checking = ref(true);
const searching = ref(false);
const searched = ref('');
const searchError = ref('');
const projects = ref<ModProject[]>([]);
let alive = true;
const visibleMods = computed(() =>
  props.mods.filter((mod) => mod.name.toLowerCase().includes(filter.value.toLowerCase())),
);
const enabled = computed(() => props.mods.filter((mod) => mod.enabled).length);
onMounted(async () => {
  try {
    const result = await api.modEnvironment(props.id);
    if (alive) environment.value = result;
  } catch (cause) {
    if (alive) environmentError.value = String(cause);
  } finally {
    if (alive) checking.value = false;
  }
});
onUnmounted(() => {
  alive = false;
});
async function search() {
  const term = query.value.trim();
  if (!term || searching.value || !environment.value) return;
  searching.value = true;
  searchError.value = '';
  projects.value = [];
  searched.value = term;
  try {
    const result = await api.searchMods(props.id, term);
    if (alive) projects.value = result;
  } catch (cause) {
    if (alive) searchError.value = String(cause);
  } finally {
    if (alive) searching.value = false;
  }
}
function install(project: ModProject) {
  if (props.busy) return;
  emit('start', { kind: 'install_mod', id: props.id, project: project.project_id });
}
const downloads = (value: number) =>
  new Intl.NumberFormat('zh-CN', { notation: 'compact', maximumFractionDigits: 1 }).format(value);
</script>
<template>
  <div class="mod-library">
    <div class="toolbar">
      <div>
        <p class="eyebrow">让世界多一点可能</p>
        <h1>模组管理</h1>
      </div>
      <button :disabled="loading || busy" @click="emit('refresh')">
        <Icon name="refresh" />刷新
      </button>
    </div>
    <div class="mod-section-switch" role="group" aria-label="模组浏览方式">
      <button :aria-pressed="view === 'local'" @click="view = 'local'">
        已安装模组<span>{{ mods.length }}</span>
      </button>
      <button :aria-pressed="view === 'discover'" @click="view = 'discover'">
        发现模组<Icon name="extension" />
      </button>
    </div>
    <template v-if="view === 'local'">
      <LocalImport :id="id" content="mods" :busy="busy" :submit="submit" />
      <div class="mod-summary">
        <span>{{ enabled }} 个启用 · {{ mods.length - enabled }} 个禁用</span
        ><span>更改会在下次启动游戏时生效</span>
      </div>
      <input
        v-model="filter"
        class="mod-search-field"
        type="search"
        aria-label="筛选已安装模组"
        placeholder="查找已安装的模组…"
      />
      <div class="list">
        <div
          v-for="mod in visibleMods"
          :key="mod.name"
          class="list-row"
          :class="{ 'mod-disabled': !mod.enabled }"
        >
          <Icon name="extension" /><span
            ><b>{{ mod.name }}</b
            ><small
              >{{ (mod.size / 1024).toFixed(0) }} KB ·
              {{ mod.enabled ? '已启用' : '已禁用' }}</small
            ></span
          >
          <button :disabled="busy" @click="emit('toggle', mod.name)">
            {{ mod.enabled ? '禁用' : '启用' }}
          </button>
        </div>
      </div>
      <div v-if="!visibleMods.length && !loading" class="mod-empty">
        <Icon name="extension" />
        <h2>{{ mods.length ? '没有匹配的模组' : '给冒险添点新鲜感' }}</h2>
        <p>
          {{
            mods.length ? '试试其他名称，或清空筛选条件。' : '前往发现页，查找适合这个实例的模组。'
          }}
        </p>
        <button v-if="mods.length" @click="filter = ''">清空筛选</button>
        <button v-else @click="view = 'discover'">发现模组<Icon name="arrow_forward" /></button>
      </div>
    </template>
    <template v-else>
      <section class="mod-discover-intro">
        <div class="mod-source-mark"><Icon name="extension" /></div>
        <div>
          <h2>寻找下一个灵感</h2>
          <p>来自 Modrinth，按当前游戏与加载器筛选。</p>
        </div>
        <span v-if="environment" class="mod-environment"
          >{{ environment.game }} · {{ environment.loader }}</span
        >
      </section>
      <p v-if="checking" role="status" class="subtitle">正在识别实例环境…</p>
      <div v-else-if="environmentError" class="mod-notice" role="status">
        <Icon name="info" />
        <div>
          <b>此实例暂不能在线安装模组</b>
          <p>{{ environmentError }}</p>
          <button @click="emit('components')">查看加载器<Icon name="arrow_forward" /></button>
        </div>
      </div>
      <template v-else>
        <form class="mod-search-form" @submit.prevent="search">
          <label class="mod-search-label"
            ><Icon name="search" /><input
              v-model="query"
              type="search"
              required
              maxlength="200"
              aria-label="搜索 Modrinth 模组"
              placeholder="搜索模组名称，例如 Sodium…"
              :disabled="searching"
          /></label>
          <button class="primary" :disabled="searching || !query.trim()">
            {{ searching ? '搜索中…' : '搜索模组' }}
          </button>
        </form>
        <p class="mod-search-help">
          安装兼容版本及其必需依赖，可选依赖不会自动安装。最多显示 20 个结果。
        </p>
        <p v-if="searching" class="mod-empty" role="status">正在寻找适合你的模组…</p>
        <div v-else-if="searchError" class="mod-search-error" role="alert">
          <Icon name="info" /><span>{{ searchError }}</span
          ><button @click="search">重试搜索</button>
        </div>
        <div v-else-if="searched" class="mod-results" aria-live="polite">
          <p class="subtitle">“{{ searched }}” · {{ projects.length }} 个结果</p>
          <article v-for="project in projects" :key="project.project_id" class="mod-project">
            <div class="mod-project-icon"><Icon name="extension" /></div>
            <div class="mod-project-copy">
              <h3>{{ project.title }}</h3>
              <span class="mod-project-slug">{{ project.slug }}</span>
              <p>{{ project.description }}</p>
              <small>{{ downloads(project.downloads) }} 次下载 · Modrinth</small>
            </div>
            <button
              :disabled="busy"
              :aria-label="`安装 ${project.title}`"
              @click="install(project)"
            >
              <Icon name="download" />安装
            </button>
          </article>
          <div v-if="!projects.length" class="mod-empty">
            <Icon name="search" />
            <h2>暂时没有找到兼容结果</h2>
            <p>试试英文名称或更短的关键词。</p>
          </div>
        </div>
        <div v-else class="mod-empty">
          <Icon name="search" />
          <h2>从一个关键词开始</h2>
          <p>优化性能、探索世界，或给游戏增加一点变化。</p>
        </div>
      </template>
    </template>
  </div>
</template>
