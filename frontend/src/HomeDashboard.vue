<script setup lang="ts">
import { computed, ref, watch, nextTick, onMounted, onUnmounted } from 'vue';
import Icon from './Icon.vue';
import WorldScene from './WorldScene.vue';
import FavoriteToggle from './FavoriteToggle.vue';
import type { Snapshot, Task } from './api';
const props = defineProps<{ data: Snapshot | null; task: Task; menu: boolean }>();
const emit = defineEmits<{
  navigate: [page: 'versions' | 'download' | 'java' | 'settings'];
  launch: [id?: string];
  select: [id: string];
  inspect: [id: string];
  favorite: [id: string];
  toggleMenu: [];
}>();
const current = computed(() =>
  props.data?.instances.find((item) => item.id === props.data?.config.selected),
);
const featured = computed(() =>
  [...(props.data?.instances ?? [])]
    .sort(
      (a, b) =>
        Number(b.id === current.value?.id) - Number(a.id === current.value?.id) ||
        Number(b.favorite) - Number(a.favorite),
    )
    .slice(0, 3),
);
const memory = computed(() => {
  const config = props.data?.config;
  return config
    ? ((config.version_settings[config.selected]?.memory ?? config.memory) / 1024).toLocaleString(
        'zh-CN',
        { maximumFractionDigits: 1 },
      )
    : '—';
});
const actions = ref<HTMLElement | null>(null);
const picker = ref<HTMLElement | null>(null);
const pickerTrigger = ref<HTMLButtonElement | null>(null);
const chart = ref<HTMLElement | null>(null);
const resting = ref(document.hidden);
let parallaxFrame = 0;
function resetChart() {
  cancelAnimationFrame(parallaxFrame);
  chart.value?.style.removeProperty('--chart-x');
  chart.value?.style.removeProperty('--chart-y');
}
function moveChart(event: PointerEvent) {
  if (
    event.pointerType === 'touch' ||
    props.data?.config.reduce_motion ||
    matchMedia('(prefers-reduced-motion: reduce)').matches
  )
    return;
  const bounds = chart.value?.getBoundingClientRect();
  if (!bounds) return;
  const x = ((event.clientX - bounds.left) / bounds.width - 0.5) * 14;
  const y = ((event.clientY - bounds.top) / bounds.height - 0.5) * 10;
  cancelAnimationFrame(parallaxFrame);
  parallaxFrame = requestAnimationFrame(() => {
    chart.value?.style.setProperty('--chart-x', `${x}px`);
    chart.value?.style.setProperty('--chart-y', `${y}px`);
  });
}
function visibilityChanged() {
  resting.value = document.hidden;
  if (resting.value) resetChart();
}
watch(() => props.data?.config.reduce_motion, resetChart);
watch(
  () => props.menu,
  async (open) => {
    await nextTick();
    if (open)
      (
        picker.value?.querySelector<HTMLButtonElement>('button[aria-current="true"]') ??
        picker.value?.querySelector<HTMLButtonElement>('button')
      )?.focus();
    else pickerTrigger.value?.focus();
  },
);
function pickerKey(event: KeyboardEvent) {
  if (event.key === 'Escape' || event.key === 'Tab') {
    event.preventDefault();
    emit('toggleMenu');
    return;
  }
  if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
  event.preventDefault();
  const items = [...(picker.value?.querySelectorAll<HTMLButtonElement>('button') ?? [])];
  const index = items.indexOf(document.activeElement as HTMLButtonElement);
  items[
    event.key === 'Home'
      ? 0
      : event.key === 'End'
        ? items.length - 1
        : (index + (event.key === 'ArrowUp' ? -1 : 1) + items.length) % items.length
  ]?.focus();
}
function closeOutside(event: PointerEvent) {
  if (props.menu && event.target instanceof Node && !actions.value?.contains(event.target))
    emit('toggleMenu');
}
onMounted(() => {
  document.addEventListener('pointerdown', closeOutside);
  document.addEventListener('visibilitychange', visibilityChanged);
});
onUnmounted(() => {
  document.removeEventListener('pointerdown', closeOutside);
  document.removeEventListener('visibilitychange', visibilityChanged);
  cancelAnimationFrame(parallaxFrame);
});
</script>
<template>
  <div class="dashboard" :class="{ 'chart-resting': resting }">
    <div class="dashboard-heading">
      <div>
        <p class="eyebrow">ASTER / 你的世界星图</p>
        <h1>欢迎回来，{{ data?.config.player ?? '冒险家' }}<span class="greeting-dot">。</span></h1>
      </div>
      <span class="quiet-pill"
        ><span class="status-dot"></span
        >{{ task.running ? '旅程进行中' : '新的世界，在等你' }}</span
      >
    </div>
    <section
      ref="chart"
      class="dashboard-hero star-chart"
      :class="{ 'journey-active': task.running }"
      @pointermove="moveChart"
      @pointerleave="resetChart"
    >
      <div class="hero-art">
        <div class="chart-grid"></div>
        <div class="chart-aurora"></div>
        <WorldScene />
      </div>
      <nav class="chart-destinations" aria-label="星图快捷入口">
        <button class="chart-destination destination-library" @click="emit('navigate', 'versions')">
          <span class="destination-beacon"><Icon name="deployed_code" /></span>
          <span><small>01 / 我的坐标</small><b>游戏库</b></span>
        </button>
        <button class="chart-destination destination-new" @click="emit('navigate', 'download')">
          <span class="destination-beacon"><Icon name="add" /></span>
          <span><small>02 / 探索未知</small><b>发现新世界</b></span>
        </button>
      </nav>
      <div class="hero-copy">
        <span class="hero-kicker"><span class="tiny-star">✦</span> 你的下一段旅程</span>
        <h2>下一站，<br /><span class="hero-title-world">你的世界。</span></h2>
        <p>
          {{
            current
              ? '熟悉的世界，或全新的可能。准备好就出发。'
              : '安装一个 Minecraft 版本，开启属于你的世界。'
          }}
        </p>
        <div ref="actions" class="hero-actions">
          <Transition name="popup"
            ><div
              v-if="menu"
              ref="picker"
              class="version-popup"
              role="menu"
              aria-label="选择游戏版本"
              @keydown="pickerKey"
            >
              <h3>选择游戏版本</h3>
              <button
                v-for="instance in data?.instances"
                :key="instance.id"
                role="menuitem"
                :aria-current="instance.id === current?.id ? 'true' : undefined"
                @click="emit('select', instance.id)"
              >
                <img :src="`/hmcl/${instance.icon}.png`" alt="" /><span
                  >{{ instance.id }}<small>{{ instance.subtitle }}</small></span
                ><Icon v-if="instance.id === current?.id" name="check" />
              </button>
              <p v-if="!data?.instances.length">还没有安装的游戏</p>
              <button role="menuitem" @click="emit('navigate', 'download')">
                <Icon name="add" />安装新版本
              </button>
            </div></Transition
          >
          <div class="launch-split">
            <button
              class="launch-primary"
              :disabled="task.running || !data"
              @click="
                current
                  ? emit('launch', current.id)
                  : data?.instances.length
                    ? emit('toggleMenu')
                    : emit('navigate', 'download')
              "
            >
              <Icon name="rocket_launch" /><span
                ><b>{{
                  task.game_running
                    ? '游戏运行中'
                    : current
                      ? '启动游戏'
                      : data?.instances.length
                        ? '选择一个游戏'
                        : '安装第一个游戏'
                }}</b
                ><small>{{ current?.id ?? '从一个小小的开始' }}</small></span
              ></button
            ><button
              class="launch-menu"
              ref="pickerTrigger"
              aria-label="选择游戏版本"
              :aria-expanded="menu"
              :disabled="task.running || !data"
              @click="emit('toggleMenu')"
            >
              <Icon name="arrow_drop_down" :class="{ rotated: menu }" />
            </button>
          </div>
          <span class="hero-version">{{ current?.subtitle ?? 'Minecraft Java 版' }}</span>
        </div>
      </div>
      <div class="scene-caption"><span class="caption-line"></span> WORLD ATLAS / 星屿</div>
    </section>
    <div class="dashboard-bottom">
      <section class="home-library">
        <div class="section-heading">
          <h2>
            随时出发<span>{{ data?.instances.length ?? 0 }}</span>
          </h2>
          <button class="text-button" @click="emit('navigate', 'versions')">
            全部游戏<Icon name="arrow_forward" />
          </button>
        </div>
        <div v-if="featured.length" class="home-game-grid">
          <article
            v-for="(instance, index) in featured"
            :key="instance.id"
            :style="{ '--card-order': index }"
            class="home-game-card"
            :class="{ 'is-current': instance.id === current?.id }"
          >
            <div class="game-card-top">
              <div class="game-cover"><img :src="`/hmcl/${instance.icon}.png`" alt="" /></div>
              <FavoriteToggle
                :id="instance.id"
                :favorite="instance.favorite"
                :disabled="task.running"
                @toggle="emit('favorite', instance.id)"
              />
            </div>
            <button
              class="home-game-title"
              :disabled="task.running"
              @click="emit('inspect', instance.id)"
            >
              <b>{{ instance.id }}</b
              ><small>{{ instance.subtitle }}</small></button
            ><button
              class="card-launch"
              :disabled="task.running"
              @click="emit('launch', instance.id)"
            >
              启动游戏<Icon name="arrow_forward" />
            </button>
          </article>
          <button
            v-if="featured.length < 3"
            class="home-add-card"
            @click="emit('navigate', 'download')"
          >
            <Icon name="add" /><b>新的旅程</b><small>安装另一个游戏版本</small>
          </button>
        </div>
        <button v-else class="empty-library" @click="emit('navigate', 'download')">
          <span class="empty-orbit"><Icon name="add" /></span><b>一个空位，无限可能</b
          ><span>添加你的第一个游戏</span>
        </button>
      </section>
      <aside class="launch-brief">
        <div class="section-heading">
          <h2>出发清单</h2>
          <Icon name="check" />
        </div>
        <div class="brief-row">
          <span>当前账户</span><b>{{ data?.config.player ?? '未选择' }}</b>
        </div>
        <div class="brief-row">
          <span>游戏内存</span><b>{{ memory }} <small>GB</small></b>
        </div>
        <button class="brief-link" @click="emit('navigate', 'java')">
          <span><Icon name="deployed_code" />Java 运行环境</span
          ><Icon name="arrow_forward" /></button
        ><button class="brief-link" @click="emit('navigate', 'settings')">
          <span><Icon name="settings" />调整启动设置</span><Icon name="arrow_forward" />
        </button>
      </aside>
    </div>
  </div>
</template>
