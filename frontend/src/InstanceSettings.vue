<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import type { Config, VersionSettings } from './api';
import Icon from './Icon.vue';

const props = defineProps<{
  config: Config;
  busy: boolean;
  save: (config: Config) => Promise<boolean>;
}>();
type Key = keyof VersionSettings;
type Field = {
  key: Key;
  title: string;
  type: 'number' | 'text' | 'boolean';
  min?: number;
  max?: number;
  yes?: string;
  no?: string;
};
const groups: { title: string; fields: Field[] }[] = [
  {
    title: 'Java 与内存',
    fields: [
      { key: 'java', title: 'Java 路径', type: 'text' },
      { key: 'memory', title: '最大内存（MB）', type: 'number', min: 512, max: 65536 },
      {
        key: 'auto_java',
        title: '自动下载 Java',
        type: 'boolean',
        yes: '缺少合适的 Java 时自动下载',
        no: '不自动下载',
      },
    ],
  },
  {
    title: '游戏窗口与目录',
    fields: [
      { key: 'fullscreen', title: '窗口模式', type: 'boolean', yes: '全屏', no: '窗口' },
      { key: 'width', title: '窗口宽度', type: 'number', min: 320, max: 7680 },
      { key: 'height', title: '窗口高度', type: 'number', min: 240, max: 4320 },
      {
        key: 'isolated',
        title: '版本隔离',
        type: 'boolean',
        yes: '使用实例独立目录',
        no: '使用共享游戏目录',
      },
    ],
  },
  { title: '高级设置', fields: [{ key: 'extra_jvm', title: '额外 JVM 参数', type: 'text' }] },
];
const enabled = reactive<Record<Key, boolean>>({
  memory: false,
  java: false,
  extra_jvm: false,
  width: false,
  height: false,
  fullscreen: false,
  isolated: false,
  auto_java: false,
});
const values = reactive<Record<Key, string | number | boolean>>({
  memory: 2048,
  java: '',
  extra_jvm: '',
  width: 854,
  height: 480,
  fullscreen: false,
  isolated: true,
  auto_java: true,
});
const pending = ref(false);
const status = ref('');
const signature = computed(() =>
  JSON.stringify([
    props.config.selected,
    props.config.version_settings[props.config.selected] ?? {},
  ]),
);
watch(
  signature,
  () => {
    const settings = props.config.version_settings[props.config.selected] ?? {};
    for (const group of groups)
      for (const field of group.fields) {
        enabled[field.key] = settings[field.key] !== undefined && settings[field.key] !== null;
        values[field.key] = settings[field.key] ?? props.config[field.key];
      }
    status.value = '';
  },
  { immediate: true },
);
function inherited(field: Field): string {
  const value = props.config[field.key];
  return field.type === 'boolean'
    ? value
      ? field.yes!
      : field.no!
    : String(value) || '自动选择／无额外参数';
}
function reset() {
  for (const key of Object.keys(enabled) as Key[]) enabled[key] = false;
  status.value = '已恢复继承，保存后生效。';
}
async function submit() {
  if (pending.value || props.busy || !props.config.selected) return;
  let settings: VersionSettings = {};
  for (const group of groups)
    for (const field of group.fields) {
      settings = { ...settings, [field.key]: enabled[field.key] ? values[field.key] : null };
    }
  const versionSettings = { ...props.config.version_settings };
  if (Object.values(enabled).some(Boolean)) versionSettings[props.config.selected] = settings;
  else delete versionSettings[props.config.selected];
  pending.value = true;
  try {
    if (await props.save({ ...props.config, version_settings: versionSettings }))
      status.value = '实例设置已保存';
  } finally {
    pending.value = false;
  }
}
</script>

<template>
  <form @submit.prevent="submit">
    <h1>游戏设置</h1>
    <p class="subtitle">未单独设置的选项继承全局设置，保存后用于下一次启动。</p>
    <fieldset :disabled="busy || pending">
      <section v-for="group in groups" :key="group.title" class="panel">
        <h2>{{ group.title }}</h2>
        <div v-for="field in group.fields" :key="field.key" class="instance-setting">
          <div class="setting-label">
            <label :for="`instance-${field.key}`">{{ field.title }}</label>
            <label class="check"
              ><input
                v-model="enabled[field.key]"
                type="checkbox"
                :aria-label="`单独设置${field.title}`"
              />单独设置</label
            >
          </div>
          <template v-if="enabled[field.key]">
            <select
              v-if="field.type === 'boolean'"
              :id="`instance-${field.key}`"
              v-model="values[field.key]"
            >
              <option :value="false">{{ field.no }}</option>
              <option :value="true">{{ field.yes }}</option>
            </select>
            <input
              v-else-if="field.type === 'number'"
              :id="`instance-${field.key}`"
              v-model.number="values[field.key]"
              type="number"
              :min="field.min"
              :max="field.max"
              required
            />
            <input
              v-else
              :id="`instance-${field.key}`"
              v-model="values[field.key]"
              type="text"
              :placeholder="field.key === 'java' ? '留空自动选择适合此实例的 Java' : ''"
            />
          </template>
          <p v-else class="subtitle inherited">继承全局：{{ inherited(field) }}</p>
        </div>
        <p v-if="group.title === '游戏窗口与目录'" class="subtitle">
          切换隔离不会搬移已有存档；整合包始终使用独立目录。
        </p>
      </section>
      <div class="toolbar">
        <button class="primary" type="submit"><Icon name="check" />保存实例设置</button
        ><button type="button" @click="reset">恢复全局设置</button
        ><span class="subtitle" role="status">{{ status }}</span>
      </div>
    </fieldset>
  </form>
</template>

<style scoped>
.instance-setting {
  padding: 10px 0;
  border-top: 1px solid var(--line);
}
.setting-label {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 8px;
}
.setting-label > label:first-child {
  font-weight: 500;
}
.setting-label .check {
  margin: 0;
  white-space: nowrap;
  font-size: 12px;
}
.instance-setting > input,
.instance-setting > select {
  width: 100%;
}
.inherited {
  margin: 0;
}
</style>
