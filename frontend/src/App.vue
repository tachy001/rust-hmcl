<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, toRaw, watch } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import Icon from './Icon.vue';
import Avatar from './Avatar.vue';
import BrandMark from './BrandMark.vue';
import HomeDashboard from './HomeDashboard.vue';
import FavoriteToggle from './FavoriteToggle.vue';
import ModLibrary from './ModLibrary.vue';
import LocalImport from './LocalImport.vue';
import PackInstaller from './PackInstaller.vue';
import JavaManager from './JavaManager.vue';
import LaunchConsole from './LaunchConsole.vue';
import InstanceSettings from './InstanceSettings.vue';
import {
  api,
  instanceIcon,
  desktopAvailable,
  type Config,
  type Job,
  type Mod,
  type Pack,
  type Snapshot,
  type Task,
  type Version,
} from './api';
import { emphasizedCss } from './motion';

type Page =
  | 'home'
  | 'accounts'
  | 'versions'
  | 'download'
  | 'packs'
  | 'java'
  | 'settings'
  | 'logs'
  | 'about'
  | 'instance';
const page = ref<Page>('home'),
  tab = ref('settings'),
  transitioning = ref(false);
const data = ref<Snapshot | null>(null),
  draft = ref<Config | null>(null),
  error = ref(''),
  loading = ref(false),
  menu = ref(false);
const task = ref<Task>({
  id: 0,
  launch_id: null,
  clues: [],
  running: false,
  game_running: false,
  progress: 0,
  status: '准备就绪',
  error: null,
  logs: [],
});
const releases = ref<Version[]>([]),
  mods = ref<Mod[]>([]),
  packs = ref<Pack[]>([]);
const query = ref(''),
  loaderName = ref(''),
  forgeVersion = ref(''),
  favoritesOnly = ref(false),
  snapshots = ref(false),
  username = ref('');
interface InstancePrompt {
  kind: 'duplicate' | 'rename';
  id: string;
  name: string;
  copy_saves: boolean;
  error: string;
  pending: boolean;
}
const dialog = ref<{
  title: string;
  message: string;
  confirm?: () => Promise<void>;
  prompt?: InstancePrompt;
} | null>(null);
const instanceMenu = ref<{ id: string; x: number; y: number } | null>(null);
const menuElement = ref<HTMLElement | null>(null);
let menuTrigger: HTMLElement | null = null;
const accountError = ref('');
const settingsSaved = ref(false);
const windowSquare = ref(false);
let stopWindowResize: (() => void) | undefined;
let unmounted = false;
async function syncWindowShape() {
  const window = getCurrentWindow();
  const [maximized, fullscreen] = await Promise.all([window.isMaximized(), window.isFullscreen()]);
  windowSquare.value = maximized || fullscreen;
}
const modalElement = ref<HTMLElement | null>(null);
let previousFocus: HTMLElement | null = null;
let timer: ReturnType<typeof setInterval> | undefined,
  polling = false;
const navigation: [Page, string, string][] = [
  ['home', 'home', '首页'],
  ['accounts', 'person', '账户'],
  ['versions', 'folder', '游戏库'],
  ['download', 'download', '下载'],
  ['packs', 'archive', '整合包'],
  ['java', 'deployed_code', 'Java 管理'],
  ['settings', 'settings', '设置'],
  ['logs', 'menu', '日志'],
  ['about', 'info', '关于'],
];
const title = computed(() =>
  page.value === 'instance'
    ? `实例管理 - ${data.value?.config.selected ?? ''}`
    : (navigation.find((item) => item[0] === page.value)?.[2] ?? '首页'),
);
const current = computed(() =>
  data.value?.instances.find((item) => item.id === data.value?.config.selected),
);
watch(
  () => data.value?.config.selected,
  () => {
    loaderName.value = '';
    forgeVersion.value = '';
  },
);
const versions = computed(
  () =>
    data.value?.instances.filter(
      (item) =>
        (!favoritesOnly.value || item.favorite) &&
        `${item.id} ${item.subtitle}`.toLowerCase().includes(query.value.toLowerCase()),
    ) ?? [],
);
const remoteVersions = computed(() =>
  releases.value
    .filter(
      (item) =>
        (snapshots.value || item.type === 'release') &&
        item.id.toLowerCase().includes(query.value.toLowerCase()),
    )
    .slice(0, 150),
);
const tabs = [
  ['settings', 'settings', '游戏设置'],
  ['components', 'deployed_code', '自动安装'],
  ['mods', 'extension', '模组管理'],
  ['resources', 'texture', '资源包管理'],
] as const;
function fail(cause: unknown) {
  error.value = String(cause);
}
function applySnapshot(value: Snapshot) {
  data.value = value;
  draft.value = structuredClone(toRaw(value.config));
}
async function refresh() {
  try {
    data.value = await api.snapshot();
    draft.value = structuredClone(toRaw(data.value.config));
  } catch (cause) {
    fail(cause);
  }
}
async function save(config: Config) {
  try {
    data.value = await api.save(config);
    draft.value = structuredClone(toRaw(data.value.config));
    return true;
  } catch (cause) {
    fail(cause);
    return false;
  }
}
async function saveSettings() {
  if (draft.value) settingsSaved.value = await save(draft.value);
}
async function select(id: string) {
  if (!data.value) return false;
  const saved = await save({ ...data.value.config, selected: id });
  if (saved) menu.value = false;
  return saved;
}
async function toggleFavorite(id: string) {
  if (!data.value || task.value.running) return;
  const config = data.value.config;
  await save({
    ...config,
    favorites: config.favorites.includes(id)
      ? config.favorites.filter((item) => item !== id)
      : [...config.favorites, id],
  });
}
async function start(job: Job) {
  try {
    error.value = '';
    await api.start(job);
    task.value = await api.task();
    if (!task.value.running) {
      await refresh();
      await loadPage();
      if (task.value.error) {
        fail(task.value.error);
        return false;
      }
    }
    return true;
  } catch (cause) {
    fail(cause);
    return false;
  }
}
async function launch(id = data.value?.config.selected) {
  if (id) {
    if (id !== data.value?.config.selected && !(await select(id))) return;
    await start({ kind: 'launch', id });
  } else go('versions');
}
async function theme() {
  if (data.value) await save({ ...data.value.config, dark_mode: !data.value.config.dark_mode });
}
function go(next: Page) {
  closeInstanceMenu();
  settingsSaved.value = false;
  page.value = next;
  query.value = '';
  menu.value = false;
}
function leavePage(element: Element) {
  element.setAttribute('inert', '');
}
function ripple(event: PointerEvent) {
  if (data.value?.config.reduce_motion || matchMedia('(prefers-reduced-motion: reduce)').matches)
    return;
  const target = event.target instanceof Element ? event.target.closest('button') : null;
  if (!target || target.disabled) return;
  const rect = target.getBoundingClientRect(),
    size = Math.max(rect.width, rect.height) * 2;
  const wave = document.createElement('span');
  wave.className = 'ripple';
  wave.setAttribute('aria-hidden', 'true');
  Object.assign(wave.style, {
    width: `${size}px`,
    height: `${size}px`,
    left: `${event.clientX - rect.left - size / 2}px`,
    top: `${event.clientY - rect.top - size / 2}px`,
  });
  target.append(wave);
  wave.addEventListener('animationend', () => wave.remove(), { once: true });
  setTimeout(() => wave.remove(), 700);
}
async function inspect(id: string) {
  if (await select(id)) {
    tab.value = 'settings';
    go('instance');
  }
}
async function recoverLaunch(action: string) {
  const id = task.value.launch_id;
  if (action === 'java') {
    go('java');
    return;
  }
  if (!id || task.value.running || !data.value?.instances.some((item) => item.id === id)) return;
  if (action === 'repair') {
    dialog.value = {
      title: '校验并补全游戏文件',
      message: `将校验 ${id} 的游戏本体与依赖并下载缺失文件。${task.value.clues.some((clue) => clue.title.endsWith('安装产物需要修复')) ? '加载器依赖会使用 Java 和官方安装器在独立副本中重建。' : ''}此操作不会自动停用模组或修改游戏设置。`,
      confirm: async () => {
        await start({ kind: 'repair', id });
      },
    };
  } else {
    if (!(await select(id))) return;
    tab.value = action === 'mods' ? 'mods' : 'settings';
    go('instance');
  }
}
async function loadPage() {
  error.value = '';
  loading.value = true;
  try {
    if (page.value === 'download' && !releases.value.length) releases.value = await api.versions();
    if (page.value === 'instance' && tab.value === 'mods') mods.value = await api.mods();
    if (page.value === 'instance' && tab.value === 'resources') packs.value = await api.packs();
  } catch (cause) {
    fail(cause);
  } finally {
    loading.value = false;
  }
}
watch([page, tab], loadPage);
watch(dialog, async (value) => {
  if (value) {
    previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    await nextTick();
    const input = modalElement.value?.querySelector<HTMLInputElement>('input[type="text"]');
    if (input) {
      input.focus();
      input.select();
    } else modalElement.value?.querySelector<HTMLButtonElement>('button:last-child')?.focus();
  } else {
    await nextTick();
    previousFocus?.focus();
  }
});
function trapTab(event: KeyboardEvent) {
  const buttons = [
    ...(modalElement.value?.querySelectorAll<HTMLElement>(
      'button:not(:disabled), input:not(:disabled), select:not(:disabled)',
    ) ?? []),
  ];
  if (!buttons.length) return;
  const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
  buttons[(index + (event.shiftKey ? -1 : 1) + buttons.length) % buttons.length]?.focus();
}
function closeDialog() {
  if (!dialog.value?.prompt?.pending) dialog.value = null;
}
async function openInstanceMenu(id: string, event: MouseEvent) {
  if (task.value.running) return;
  menuTrigger = event.currentTarget instanceof HTMLElement ? event.currentTarget : null;
  if (menuTrigger && !(menuTrigger instanceof HTMLButtonElement))
    menuTrigger = menuTrigger.querySelector<HTMLButtonElement>('button:last-child');
  const rect = menuTrigger?.getBoundingClientRect();
  instanceMenu.value = {
    id,
    x: Math.max(
      8,
      Math.min(
        event.type === 'contextmenu' ? event.clientX : (rect?.right ?? 240) - 240,
        innerWidth - 248,
      ),
    ),
    y: Math.max(
      44,
      Math.min(
        event.type === 'contextmenu' ? event.clientY : (rect?.bottom ?? 44),
        innerHeight - 228,
      ),
    ),
  };
  await nextTick();
  menuElement.value?.querySelector<HTMLButtonElement>('button')?.focus();
}
function closeInstanceMenu() {
  if (!instanceMenu.value) return;
  instanceMenu.value = null;
  menuTrigger?.focus();
}
function menuKey(event: KeyboardEvent) {
  if (event.key === 'Escape' || event.key === 'Tab') {
    event.preventDefault();
    closeInstanceMenu();
  } else if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
    event.preventDefault();
    const items = [...(menuElement.value?.querySelectorAll<HTMLButtonElement>('button') ?? [])];
    const current = items.indexOf(document.activeElement as HTMLButtonElement);
    const index =
      event.key === 'Home'
        ? 0
        : event.key === 'End'
          ? items.length - 1
          : (current + (event.key === 'ArrowUp' ? -1 : 1) + items.length) % items.length;
    items[index]?.focus();
  }
}
function instanceAction(action: 'manage' | 'launch' | 'duplicate' | 'rename', id: string) {
  closeInstanceMenu();
  if (action === 'manage') {
    void inspect(id);
    return;
  }
  if (action === 'launch') {
    void launch(id);
    return;
  }
  dialog.value = {
    title: action === 'duplicate' ? '复制实例' : '重命名实例',
    message:
      action === 'duplicate'
        ? `复制“${id}”的设置和游戏文件。副本使用独立游戏目录，原实例保持不变。`
        : `为“${id}”输入新名称。存档和实例设置将随实例保留。`,
    prompt: { kind: action, id, name: id, copy_saves: false, error: '', pending: false },
  };
}
async function submitInstance() {
  const prompt = dialog.value?.prompt;
  if (!prompt || prompt.pending) return;
  const stem = prompt.name.split('.')[0]?.toUpperCase() ?? '';
  if (
    !prompt.name ||
    prompt.name.length > 100 ||
    /[<>:"/\\|?*\u0000-\u001f\u007f]/.test(prompt.name) ||
    /[. ]$/.test(prompt.name) ||
    /^(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])$/.test(stem)
  ) {
    prompt.error = '实例名称应为 1–100 字，不能使用路径符号、系统保留名称或以点、空格结尾。';
    return;
  }
  if (prompt.name === prompt.id || data.value?.instances.some((item) => item.id === prompt.name)) {
    prompt.error =
      prompt.name === prompt.id ? '新名称与原名称相同。' : '同名实例已存在，不能覆盖。';
    return;
  }
  prompt.pending = true;
  prompt.error = '';
  try {
    await api.start(
      prompt.kind === 'duplicate'
        ? { kind: 'duplicate', id: prompt.id, name: prompt.name, copy_saves: prompt.copy_saves }
        : { kind: 'rename', id: prompt.id, name: prompt.name },
    );
    task.value = await api.task();
    dialog.value = null;
    if (!task.value.running) {
      await refresh();
      await loadPage();
      if (task.value.error) fail(task.value.error);
    }
  } catch (cause) {
    prompt.error = String(cause);
  } finally {
    prompt.pending = false;
  }
}
async function account() {
  if (!data.value) return;
  if (!/^[A-Za-z0-9_]{1,16}$/.test(username.value)) {
    accountError.value = '请输入 1–16 位字母、数字或下划线。';
    return;
  }
  if (
    await save({
      ...data.value.config,
      player: username.value,
      accounts: [...data.value.config.accounts, username.value],
    })
  ) {
    username.value = '';
    accountError.value = '';
  }
}

async function toggleMod(name: string) {
  try {
    mods.value = await api.toggleMod(name);
  } catch (cause) {
    fail(cause);
  }
}
async function togglePack(pack: Pack, confirmed = false) {
  if (!pack.enabled && !pack.compatible && !confirmed) {
    dialog.value = {
      title: '资源包兼容性提示',
      message: `${pack.title}\n${pack.warning ?? '可能不兼容当前游戏'}，仍要启用吗？`,
      confirm: () => togglePack(pack, true),
    };
    return;
  }
  try {
    packs.value = await api.enablePacks([pack.name], !pack.enabled, confirmed);
  } catch (cause) {
    fail(cause);
  }
}
function stop() {
  if (task.value.game_running)
    dialog.value = {
      title: '结束游戏',
      message: '确定结束正在运行的 Minecraft 吗？',
      confirm: async () => {
        await api.cancel();
      },
    };
  else void api.cancel().catch(fail);
}
async function confirmDialog() {
  const action = dialog.value?.confirm;
  dialog.value = null;
  if (action)
    try {
      await action();
    } catch (cause) {
      fail(cause);
    }
}
function windowAction(action: 'minimize' | 'toggleMaximize' | 'close') {
  if (!desktopAvailable()) return;
  if (action === 'close' && task.value.running) {
    dialog.value = { title: '任务仍在运行', message: '请先结束当前任务，再关闭启动器。' };
    return;
  }
  void getCurrentWindow()
    [action]()
    .then(() => (action === 'toggleMaximize' ? syncWindowShape() : undefined))
    .catch(fail);
}
async function poll() {
  if (polling || !desktopAvailable()) return;
  polling = true;
  try {
    const previous = task.value;
    task.value = await api.task();
    if (previous.running && !task.value.running) {
      await refresh();
      await loadPage();
    }
    if (task.value.error && task.value.error !== previous.error) error.value = task.value.error;
  } catch (cause) {
    fail(cause);
  } finally {
    polling = false;
  }
}
onMounted(async () => {
  if (!desktopAvailable()) {
    error.value = '浏览器预览未连接桌面后端。请使用 npm run desktop 启动桌面版。';
    return;
  }
  await refresh();
  await poll();
  timer = setInterval(poll, 500);
  try {
    const stop = await getCurrentWindow().onResized(() => {
      void syncWindowShape().catch(fail);
    });
    if (unmounted) stop();
    else stopWindowResize = stop;
    await syncWindowShape();
  } catch (cause) {
    fail(cause);
  }
});
onUnmounted(() => {
  unmounted = true;
  stopWindowResize?.();
  if (timer) clearInterval(timer);
});
</script>

<template>
  <div
    class="launcher"
    :class="{
      dark: data?.config.dark_mode ?? true,
      reduced: data?.config.reduce_motion,
      'window-square': windowSquare,
    }"
    :style="{ '--ease-emphasized': emphasizedCss }"
    @pointerdown="ripple"
  >
    <div class="application" :inert="!!dialog">
      <header
        class="titlebar"
        @dblclick.self="windowAction('toggleMaximize')"
        data-tauri-drag-region
      >
        <button
          v-if="page !== 'home'"
          class="window-button back"
          aria-label="返回首页"
          @click="go('home')"
        >
          <Icon name="arrow_back" />
        </button>
        <BrandMark v-else class="title-logo" />
        <span data-tauri-drag-region>{{ page === 'home' ? '星屿 Aster / 工作台' : title }}</span>
        <div class="window-actions">
          <button
            class="window-button"
            aria-label="切换主题"
            :disabled="!data || task.running"
            @click="theme"
          >
            <Icon name="wb_sunny" /></button
          ><button class="window-button" aria-label="最小化" @click="windowAction('minimize')">
            −</button
          ><button
            class="window-button"
            aria-label="最大化"
            @click="windowAction('toggleMaximize')"
          >
            □</button
          ><button class="window-button close" aria-label="关闭窗口" @click="windowAction('close')">
            <Icon name="close" />
          </button>
        </div>
      </header>
      <aside class="sidebar">
        <div class="sidebar-brand">
          <BrandMark /><span>星屿<small>ASTER LAUNCHER</small></span>
        </div>
        <div class="sidebar-section">{{ page === 'instance' ? '当前游戏' : '工作空间' }}</div>
        <template v-if="page === 'instance'">
          <button
            v-for="item in tabs"
            :key="item[0]"
            :class="{ selected: tab === item[0] }"
            @click="tab = item[0]"
          >
            <Icon :name="item[1]" /><span>{{ item[2] }}</span>
          </button>
          <div class="nav-spacer"></div>
          <button
            :disabled="task.running || !current"
            @click="current && instanceAction('rename', current.id)"
          >
            <Icon name="edit" /><span>重命名实例</span>
          </button>
          <button
            :disabled="task.running || !current"
            @click="current && instanceAction('duplicate', current.id)"
          >
            <Icon name="folder_copy" /><span>复制实例</span>
          </button>
          <button :disabled="task.running" @click="launch()">
            <Icon name="rocket_launch" /><span>测试游戏</span></button
          ><button @click="go('versions')"><Icon name="folder" /><span>返回游戏列表</span></button>
        </template>
        <template v-else>
          <button class="account-nav" aria-label="账户" @click="go('accounts')">
            <Avatar v-if="data" :skin="data.avatars[data.config.player] ?? 'wide/steve'" /><Icon
              v-else
              name="person"
            /><span
              ><b>{{ data?.config.player ?? '账户' }}</b
              ><small>离线账户</small></span
            >
          </button>
          <div class="nav-divider"></div>
          <button
            v-for="item in navigation.filter(
              (item) => item[0] !== 'accounts' && item[0] !== 'about',
            )"
            :key="item[0]"
            :class="{ selected: page === item[0] }"
            @click="go(item[0])"
          >
            <Icon :name="item[1]" /><span>{{ item[2] }}</span>
          </button>
          <div class="nav-spacer"></div>
          <div class="sidebar-footer">
            <span class="status-dot"></span>ASTER <small>开发预览版</small
            ><button aria-label="关于" @click="go('about')"><Icon name="info" /></button>
          </div>
        </template>
      </aside>
      <main class="content" :class="{ home: page === 'home' }">
        <div v-if="error" class="error-banner" role="alert">
          <Icon name="error" /><span>{{ error }}</span
          ><button v-if="task.error && error === task.error && page !== 'logs'" @click="go('logs')">
            查看任务日志</button
          ><button aria-label="关闭提示" @click="error = ''">×</button>
        </div>
        <Transition
          name="page"
          @before-leave="leavePage"
          @before-enter="transitioning = true"
          @after-enter="transitioning = false"
          @enter-cancelled="transitioning = false"
        >
          <section :key="`${page}/${tab}`" class="page" :inert="transitioning || !!dialog">
            <template v-if="page === 'home'">
              <HomeDashboard
                :data="data"
                :task="task"
                :menu="menu"
                @navigate="go"
                @launch="launch"
                @select="select"
                @inspect="inspect"
                @favorite="toggleFavorite"
                @toggle-menu="menu = !menu"
              />
            </template>
            <template v-else-if="page === 'accounts'">
              <h1>账户</h1>
              <p class="subtitle">选择用于启动 Minecraft 的账户。</p>
              <div class="list">
                <button
                  v-for="name in data?.config.accounts"
                  :key="name"
                  class="list-row"
                  :class="{ active: name === data?.config.player }"
                  :disabled="task.running"
                  @click="data && save({ ...data.config, player: name })"
                >
                  <Avatar :skin="data?.avatars[name] ?? 'wide/steve'" /><span
                    ><b>{{ name }}</b
                    ><small>离线账户</small></span
                  ><Icon v-if="name === data?.config.player" name="check" />
                </button>
              </div>
              <form class="panel compact" @submit.prevent="account">
                <h2>添加离线账户</h2>
                <div class="inline">
                  <input
                    v-model="username"
                    aria-label="离线用户名"
                    placeholder="用户名"
                    maxlength="16"
                    :disabled="task.running"
                  /><button class="primary" :disabled="task.running || !data">添加账户</button>
                </div>
                <p v-if="accountError" class="field-error">{{ accountError }}</p>
                <p class="subtitle">目前支持离线账户；Microsoft 登录与第三方认证暂未提供。</p>
              </form>
            </template>
            <template v-else-if="page === 'versions'">
              <div class="toolbar">
                <h1>
                  游戏库 <span class="heading-count">{{ data?.instances.length ?? 0 }}</span>
                </h1>
                <button :disabled="loading || task.running" @click="refresh">
                  <Icon name="refresh" />刷新</button
                ><button @click="go('download')"><Icon name="add" />安装游戏</button>
              </div>
              <div class="library-controls">
                <input
                  v-model="query"
                  class="search"
                  aria-label="搜索游戏版本"
                  placeholder="搜索游戏版本"
                /><button
                  class="filter-chip"
                  :class="{ active: favoritesOnly }"
                  :aria-pressed="favoritesOnly"
                  @click="favoritesOnly = !favoritesOnly"
                >
                  只看收藏
                </button>
              </div>
              <div class="list library-list">
                <div
                  v-for="instance in versions"
                  :key="instance.id"
                  class="list-row"
                  :class="{ active: instance.id === data?.config.selected }"
                  @contextmenu.prevent="openInstanceMenu(instance.id, $event)"
                >
                  <input
                    type="radio"
                    name="instance"
                    :aria-label="`选择 ${instance.id}`"
                    :checked="instance.id === data?.config.selected"
                    :disabled="task.running"
                    @click.prevent="select(instance.id)"
                  /><img :src="instanceIcon(instance.icon)" alt="" /><button
                    class="row-content"
                    :disabled="task.running"
                    @click="inspect(instance.id)"
                  >
                    <b>{{ instance.id }}</b
                    ><small>{{ instance.subtitle }}</small></button
                  ><FavoriteToggle
                    :id="instance.id"
                    :favorite="instance.favorite"
                    :disabled="task.running"
                    @toggle="toggleFavorite(instance.id)"
                  /><button
                    class="icon-button"
                    :aria-label="`启动 ${instance.id}`"
                    :disabled="task.running"
                    @click="launch(instance.id)"
                  >
                    <Icon name="rocket_launch" /></button
                  ><button
                    class="icon-button"
                    :aria-label="`管理 ${instance.id}`"
                    :disabled="task.running"
                    @click="inspect(instance.id)"
                  >
                    <Icon name="settings" /></button
                  ><button
                    class="icon-button"
                    :aria-label="`${instance.id}更多操作`"
                    aria-haspopup="menu"
                    :aria-expanded="instanceMenu?.id === instance.id"
                    :disabled="task.running"
                    @click="openInstanceMenu(instance.id, $event)"
                  >
                    <Icon name="more_vert" />
                  </button>
                </div>
              </div>
              <div v-if="!versions.length" class="empty">
                <Icon name="folder" />
                <h2>
                  {{ query || favoritesOnly ? '这里暂时没有匹配的游戏' : '给你的冒险，留一个位置' }}
                </h2>
                <p>
                  {{
                    query || favoritesOnly
                      ? '换个关键词，或显示全部游戏。'
                      : '安装 Minecraft，或在设置中选择已有游戏目录。'
                  }}
                </p>
                <button
                  v-if="query || favoritesOnly"
                  @click="
                    query = '';
                    favoritesOnly = false;
                  "
                >
                  显示全部游戏</button
                ><button v-else class="primary" @click="go('download')">
                  <Icon name="add" />安装第一个游戏
                </button>
              </div>
            </template>
            <template v-else-if="page === 'download'">
              <h1>下载游戏</h1>
              <div class="toolbar">
                <input
                  v-model="query"
                  aria-label="搜索可下载版本"
                  placeholder="搜索 Minecraft 版本"
                /><label class="check"
                  ><input v-model="snapshots" type="checkbox" />显示快照及历史版本</label
                ><button
                  :disabled="loading"
                  @click="
                    releases = [];
                    loadPage();
                  "
                >
                  <Icon name="refresh" />刷新
                </button>
              </div>
              <p v-if="loading" class="loading">正在获取官方版本列表…</p>
              <div class="list">
                <div v-for="version in remoteVersions" :key="version.id" class="list-row">
                  <img src="/hmcl/grass.png" alt="" /><span
                    ><b>{{ version.id }}</b
                    ><small>{{ version.type === 'release' ? '正式版' : version.type }}</small></span
                  ><button
                    :disabled="task.running"
                    @click="start({ kind: 'install', id: version.id })"
                  >
                    <Icon name="download" />安装
                  </button>
                </div>
              </div>
            </template>
            <PackInstaller
              v-else-if="page === 'packs'"
              :task="task"
              :instances="data?.instances ?? []"
              :submit="start"
              @library="go('versions')"
            />
            <JavaManager
              v-else-if="page === 'java' && data"
              :config="data.config"
              :task="task"
              :save="save"
              :submit="start"
              @snapshot="applySnapshot"
            />
            <template v-else-if="page === 'settings' && draft">
              <h1>启动器设置</h1>
              <form
                @submit.prevent="saveSettings"
                @input="settingsSaved = false"
                @change="settingsSaved = false"
              >
                <fieldset :disabled="task.running">
                  <div class="panel">
                    <h2>游戏目录</h2>
                    <label>目录路径<input v-model="draft.game_dir" aria-label="游戏目录" /></label
                    ><label class="switch-row"
                      ><input v-model="draft.isolated" type="checkbox" />版本隔离</label
                    >
                  </div>
                  <div class="panel">
                    <h2>游戏启动</h2>
                    <div class="form-grid">
                      <label
                        >最大内存（MB）<input
                          v-model.number="draft.memory"
                          type="number"
                          min="512"
                          max="65536" /></label
                      ><label
                        >下载并发<input
                          v-model.number="draft.download_threads"
                          type="number"
                          min="1"
                          max="32" /></label
                      ><label
                        >窗口宽度<input
                          v-model.number="draft.width"
                          type="number"
                          min="320"
                          max="7680" /></label
                      ><label
                        >窗口高度<input
                          v-model.number="draft.height"
                          type="number"
                          min="240"
                          max="4320"
                      /></label>
                    </div>
                    <label>Java 路径<input v-model="draft.java" placeholder="留空自动选择" /></label
                    ><label>额外 JVM 参数<input v-model="draft.extra_jvm" /></label
                    ><label class="switch-row"
                      ><input v-model="draft.fullscreen" type="checkbox" />全屏启动</label
                    ><label class="switch-row"
                      ><input v-model="draft.auto_java" type="checkbox" />缺少 Java
                      时自动下载</label
                    >
                  </div>
                  <div class="panel">
                    <h2>外观与动画</h2>
                    <label class="switch-row"
                      ><input v-model="draft.dark_mode" type="checkbox" />深色主题</label
                    ><label class="switch-row"
                      ><input
                        v-model="draft.reduce_motion"
                        type="checkbox"
                      />减少动画和动态效果</label
                    >
                  </div>
                  <div class="form-actions">
                    <button class="primary" type="submit"><Icon name="check" />保存设置</button>
                    <span v-if="settingsSaved" class="saved-message" role="status"
                      ><Icon name="check" />设置已保存</span
                    >
                  </div>
                </fieldset>
              </form>
            </template>
            <template v-else-if="page === 'instance'">
              <InstanceSettings
                v-if="tab === 'settings' && data"
                :config="data.config"
                :busy="task.running"
                :save="save"
              />
              <div v-if="tab === 'components'" class="panel">
                <h2>游戏组件</h2>
                <p>{{ current?.subtitle }}</p>
                <label class="loader-name-field"
                  >新实例名称（可选）<input
                    v-model="loaderName"
                    aria-label="加载器实例名称"
                    :disabled="task.running"
                    placeholder="留空使用加载器默认名称"
                /></label>
                <p class="loader-install-note">
                  安装会创建新的加载器实例，保留当前游戏。已有同名实例不会覆盖，可在游戏库中校验。
                </p>
                <div class="toolbar">
                  <button
                    :disabled="task.running"
                    @click="
                      data &&
                      start({
                        kind: 'fabric',
                        id: data.config.selected,
                        name: loaderName.trim() || undefined,
                      })
                    "
                  >
                    <img src="/hmcl/fabric.png" alt="" />安装 Fabric</button
                  ><button
                    :disabled="task.running"
                    @click="
                      data &&
                      start({
                        kind: 'quilt',
                        id: data.config.selected,
                        name: loaderName.trim() || undefined,
                      })
                    "
                  >
                    <img src="/hmcl/quilt.png" alt="" />安装 Quilt
                  </button>
                </div>
                <section class="forge-installer">
                  <div>
                    <img src="/aster/forge.svg" alt="" /><span
                      ><b>Forge</b><small>现代安装器 · Java 17 及以上</small></span
                    >
                  </div>
                  <p>
                    安装时使用游戏所需的
                    Java，可按设置自动下载。安装器在独立副本中运行，新实例默认使用独立游戏目录。
                  </p>
                  <label
                    >Forge 版本（可选）<input
                      v-model="forgeVersion"
                      aria-label="Forge 版本"
                      :disabled="task.running"
                      placeholder="留空使用官方推荐版本"
                  /></label>
                  <button
                    :disabled="task.running"
                    @click="
                      data &&
                      start({
                        kind: 'forge',
                        id: data.config.selected,
                        name: loaderName.trim() || undefined,
                        version: forgeVersion.trim() || undefined,
                      })
                    "
                  >
                    <Icon name="download" />安装 Forge
                  </button>
                </section>
                <p class="subtitle">NeoForge 安装尚未实现。</p>
              </div>
              <ModLibrary
                v-if="tab === 'mods' && data"
                :key="data.config.selected"
                :id="data.config.selected"
                :mods="mods"
                :busy="task.running"
                :loading="loading"
                :submit="start"
                @refresh="loadPage"
                @toggle="toggleMod"
                @start="start"
                @components="tab = 'components'"
              />
              <template v-if="tab === 'resources'"
                ><div class="toolbar">
                  <h1>资源包管理</h1>
                  <button :disabled="loading" @click="loadPage"><Icon name="refresh" />刷新</button>
                </div>
                <LocalImport
                  v-if="data"
                  :key="data.config.selected"
                  :id="data.config.selected"
                  content="resources"
                  :busy="task.running"
                  :submit="start"
                />
                <div class="list">
                  <div v-for="pack in packs" :key="pack.name" class="list-row">
                    <input
                      type="checkbox"
                      :aria-label="`启用 ${pack.title}`"
                      :checked="pack.enabled"
                      :disabled="task.running"
                      @click.prevent="togglePack(pack)"
                    /><Icon name="texture" /><span
                      ><b>{{ pack.title }}</b
                      ><small>{{ pack.summary }}</small
                      ><small v-if="pack.warning" class="warning">{{ pack.warning }}</small></span
                    ><button
                      class="icon-button"
                      :aria-label="`${pack.title}详情`"
                      @click="dialog = { title: pack.title, message: pack.description }"
                    >
                      <Icon name="info" />
                    </button>
                  </div>
                </div>
                <p v-if="!packs.length && !loading" class="empty">没有资源包</p></template
              >
            </template>
            <LaunchConsole
              v-else-if="page === 'logs'"
              :task="task"
              :recoverable="!!data?.instances.some((item) => item.id === task.launch_id)"
              @recover="recoverLaunch"
            />
            <template v-else-if="page === 'about'"
              ><div class="about">
                <BrandMark />
                <p class="eyebrow">A LITTLE SPACE. INFINITE WORLDS.</p>
                <h1>星屿 Aster</h1>
                <p>留一片天地，给你的想象。</p>
                <p class="subtitle">
                  一个正在成长的独立 Minecraft 启动器。开发预览版，部分功能仍在完善。
                </p>
                <div class="about-credits">
                  <h2>致谢</h2>
                  <p>
                    底层重构与交互研究参考 Hello Minecraft! Launcher 和
                    PCL；部分游戏图标、默认皮肤和通用图标沿用 HMCL
                    资源及其许可。星屿的品牌、界面与浮岛插画为本项目原创。
                  </p>
                  <small
                    >感谢 PCL 作者龙腾猫跃及其贡献者公开研究资料。本项目与上述启动器及
                    Mojang、Microsoft 无隶属关系。</small
                  >
                </div>
              </div></template
            >
          </section></Transition
        >
      </main>
      <footer class="statusbar">
        <Icon :name="task.running ? 'download' : 'check'" /><span role="status">{{
          task.status || '准备就绪'
        }}</span
        ><template v-if="task.running"
          ><progress :value="task.progress > 0 ? task.progress : undefined" max="1"></progress
          ><button @click="stop">{{ task.game_running ? '结束游戏' : '取消' }}</button></template
        >
      </footer>
    </div>
    <div
      v-if="instanceMenu"
      class="menu-backdrop"
      @pointerdown.prevent="closeInstanceMenu"
      @contextmenu.prevent="closeInstanceMenu"
    ></div>
    <Transition name="popup" appear>
      <div
        v-if="instanceMenu"
        ref="menuElement"
        class="instance-popup"
        role="menu"
        :aria-label="`${instanceMenu.id}操作`"
        :style="{ left: `${instanceMenu.x}px`, top: `${instanceMenu.y}px` }"
        @keydown="menuKey"
      >
        <button role="menuitem" @click="instanceAction('launch', instanceMenu.id)">
          <Icon name="rocket_launch" />测试游戏
        </button>
        <button role="menuitem" @click="instanceAction('manage', instanceMenu.id)">
          <Icon name="settings" />游戏管理
        </button>
        <div class="nav-divider"></div>
        <button role="menuitem" @click="instanceAction('rename', instanceMenu.id)">
          <Icon name="edit" />重命名实例
        </button>
        <button role="menuitem" @click="instanceAction('duplicate', instanceMenu.id)">
          <Icon name="folder_copy" />复制实例
        </button>
      </div>
    </Transition>
    <Transition name="dialog"
      ><div
        v-if="dialog"
        ref="modalElement"
        class="modal-overlay"
        @keydown.esc.stop="closeDialog"
        @keydown.tab.prevent="trapTab"
        tabindex="-1"
      >
        <section class="modal" role="dialog" aria-modal="true" :aria-label="dialog.title">
          <h2>{{ dialog.title }}</h2>
          <p>{{ dialog.message }}</p>
          <form v-if="dialog.prompt" @submit.prevent="submitInstance">
            <label class="prompt-field"
              >新实例名称<input
                v-model="dialog.prompt.name"
                @input="dialog.prompt.error = ''"
                type="text"
                aria-label="新实例名称"
                maxlength="100"
                :disabled="dialog.prompt.pending"
                :aria-invalid="!!dialog.prompt.error"
                aria-describedby="instance-name-error"
            /></label>
            <label v-if="dialog.prompt.kind === 'duplicate'" class="check"
              ><input
                v-model="dialog.prompt.copy_saves"
                type="checkbox"
                :disabled="dialog.prompt.pending"
              />复制存档</label
            >
            <p v-if="dialog.prompt.error" id="instance-name-error" role="alert" class="field-error">
              {{ dialog.prompt.error }}
            </p>
            <div class="toolbar">
              <button type="submit" class="primary" :disabled="dialog.prompt.pending">
                {{ dialog.prompt.pending ? '正在提交…' : '确认' }}</button
              ><button type="button" :disabled="dialog.prompt.pending" @click="closeDialog">
                取消
              </button>
            </div>
          </form>
          <div v-else class="toolbar">
            <button v-if="dialog.confirm" class="primary" @click="confirmDialog">确认</button
            ><button @click="closeDialog">{{ dialog.confirm ? '取消' : '关闭' }}</button>
          </div>
        </section>
      </div></Transition
    >
  </div>
</template>
