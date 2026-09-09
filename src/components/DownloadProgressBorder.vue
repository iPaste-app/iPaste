<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{ progress: number | null; label: string }>();
const percent = computed(() => props.progress === null ? undefined : Math.min(100, Math.max(0, props.progress)));
</script>

<template>
  <span
    class="download-progress-border"
    :class="{ 'download-progress-border-pending': percent === undefined }"
    :style="{ '--download-progress': `${percent ?? 25}%` }"
    role="progressbar"
    :aria-label="label"
    :aria-valuenow="percent"
    :aria-valuemin="0"
    :aria-valuemax="100"
  />
</template>

<style scoped>
.download-progress-border {
  position: absolute;
  inset: -1px;
  border-radius: inherit;
  pointer-events: none;
  padding: 2px;
  background: conic-gradient(#0d9488 var(--download-progress), #0d948826 0);
  mask: linear-gradient(#fff 0 0) content-box, linear-gradient(#fff 0 0);
  mask-composite: exclude;
}

.download-progress-border-pending {
  animation: download-border-pulse 1.2s ease-in-out infinite alternate;
}

@keyframes download-border-pulse {
  to { opacity: 0.35; }
}

@media (prefers-reduced-motion: reduce) {
  .download-progress-border-pending { animation: none; }
}
</style>
