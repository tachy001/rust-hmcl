<script setup lang="ts">
defineProps<{ id: string; favorite: boolean; disabled?: boolean }>();
defineEmits<{ toggle: [] }>();
</script>
<template>
  <button
    class="favorite-button"
    :class="{ 'is-favorite': favorite }"
    :aria-label="`${favorite ? '取消收藏' : '收藏'} ${id}`"
    :aria-pressed="favorite"
    :disabled="disabled"
    @click="$emit('toggle')"
  >
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="m12 3 2.8 5.7 6.2.9-4.5 4.4 1 6.2-5.5-3-5.5 3 1-6.2L3 9.6l6.2-.9Z" />
    </svg>
  </button>
</template>
