<script setup lang="ts">
import { computed, ref } from "vue";
import { ChevronRight, LoaderCircle } from "lucide-vue-next";
import { t } from "../i18n";
import { ipasteApi } from "../lib/ipasteApi";
import type { OcrModelIssue } from "../lib/ocrError";

const props = defineProps<{ issue: OcrModelIssue }>();
const opening = ref(false);
const openFailed = ref(false);
const isRepair = computed(() => props.issue.kind === "repair");

async function openSettings() {
  if (opening.value) return;
  opening.value = true;
  openFailed.value = false;
  try {
    await ipasteApi.showSettings("ocr");
  } catch {
    openFailed.value = true;
  } finally {
    opening.value = false;
  }
}
</script>

<template>
  <div class="ocr-setup-guide">
    <p>{{ t(isRepair ? "ocr.setup.repairHint" : "ocr.setup.downloadHint") }}</p>
    <button type="button" class="settings-action-button settings-action-button-primary" :disabled="opening" :aria-busy="opening" @click="openSettings">
      <LoaderCircle v-if="opening" class="size-4 update-spin" aria-hidden="true" />
      <span>{{ t(opening ? "ocr.setup.opening" : isRepair ? "ocr.setup.repairAction" : "ocr.setup.downloadAction") }}</span>
      <ChevronRight v-if="!opening" class="size-4" aria-hidden="true" />
    </button>
    <p v-if="openFailed" class="ocr-setup-open-error" role="alert">{{ t("ocr.setup.openError") }}</p>
    <details v-if="isRepair && issue.missingFiles.length" class="ocr-setup-details">
      <summary>{{ t("ocr.setup.details") }}</summary>
      <p>{{ issue.missingFiles.join("\n") }}</p>
    </details>
  </div>
</template>

<style scoped>
.ocr-setup-guide {
  display: flex;
  min-height: 0;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.875rem;
  overflow-y: auto;
  color: #475569;
  font-size: 0.8125rem;
  line-height: 1.65;
}

.ocr-setup-guide p {
  margin: 0;
}

.ocr-setup-open-error {
  color: #b91c1c;
}

.ocr-setup-details {
  width: 100%;
  color: #64748b;
  font-size: 0.75rem;
}

.ocr-setup-details summary {
  cursor: pointer;
}

.ocr-setup-details summary:focus-visible {
  outline: 2px solid #0d9488;
  outline-offset: 2px;
}

.ocr-setup-details p {
  margin-top: 0.5rem;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}
</style>
