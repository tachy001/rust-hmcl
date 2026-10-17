<script setup lang="ts">
defineProps<{ name: string }>();
</script>
<template>
  <span
    aria-hidden="true"
    class="icon"
    :style="{ '--icon': `url('/hmcl/icons/${name}.svg')` }"
  ></span>
</template>
