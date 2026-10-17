<script setup lang="ts">
defineProps<{ skin: string }>();
</script>
<template>
  <span class="avatar" aria-hidden="true" :style="{ '--skin': `url('/hmcl/skins/${skin}.png')` }"
    ><span class="avatar-face"></span><span class="avatar-hat"></span
  ></span>
</template>
<style scoped>
.avatar {
  display: inline-block;
  position: relative;
  width: 32px;
  height: 32px;
  flex: 0 0 32px;
  image-rendering: pixelated;
}
.avatar-face,
.avatar-hat {
  position: absolute;
  inset: 0;
  background-image: var(--skin);
  background-size: 256px 256px;
  background-repeat: no-repeat;
}
.avatar-face {
  background-position: -32px -32px;
}
.avatar-hat {
  background-position: -160px -32px;
  transform: scale(1.08);
}
</style>
