<script setup lang="ts">
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import { computed, ref, watch } from "vue";
import { AlertCircle, Check, CheckCircle2, ChevronDown, Download, ExternalLink, FolderOpen, LoaderCircle, Trash2 } from "lucide-vue-next";
import { OCR_MODELS, formatOcrBytes, useOcrModels } from "../composables/useOcrModels";
import { t } from "../i18n";
import type { OcrMode } from "../types";

const props = defineProps<{ refreshKey?: number }>();
const {
  statuses, loading, removing, operation, progress, failure, error, notice,
  bytesPerSecond, busy, hasModels, installDir, percent,
  installed, inUse, needsRepair, load, choose, removeAll,
} = useOcrModels();
watch(() => props.refreshKey, () => {
  if (!busy.value) void load();
});
const confirmRemoval = ref(false);
const removalText = computed(() => t(removing.value ? "ocr.deleting" : confirmRemoval.value ? "ocr.confirmRemove" : "ocr.deleteResources"));
const modelSource = "https://www.modelscope.cn/models/RapidAI/RapidOCR";
const phaseText = computed(() => {
  if (progress.value?.phase === "fetchingManifest") return t("ocr.install.preparing");
  if (progress.value?.phase === "verifying") return t("ocr.install.verifying");
  if (progress.value?.phase === "completed") return t("ocr.install.finishing");
  return t("ocr.install.downloading");
});

function actionText(mode: OcrMode) {
  if (operation.value?.mode === mode) return t("ocr.install.finishing");
  if (failure.value?.mode === mode) return t("ocr.retry");
  if (needsRepair(mode)) return t("ocr.repairModel");
  return installed(mode) ? t("ocr.useModel") : t("ocr.downloadAndUse");
}

async function handleRemoval() {
  if (busy.value || !hasModels.value) return;
  if (!confirmRemoval.value) {
    confirmRemoval.value = true;
    return;
  }
  confirmRemoval.value = false;
  await removeAll();
}

async function openDirectory() {
  try { await openPath(installDir.value); }
  catch (cause) { error.value = String(cause); }
}

async function openSource() {
  try {
    if ("__TAURI_INTERNALS__" in window) await openUrl(modelSource);
    else window.open(modelSource, "_blank", "noopener,noreferrer");
  } catch (cause) { error.value = String(cause); }
}

</script>

<template>
  <section class="settings-panel settings-column-panel ocr-settings" :aria-label="t('ocr.modelSelectionTitle')">
    <h2 class="ocr-heading">{{ t("ocr.modelSelectionTitle") }}</h2>

    <div class="ocr-cards">
      <article
        v-for="model in OCR_MODELS"
        :key="model.mode"
        class="ocr-card"
        :class="{ 'ocr-card-active': inUse(model.mode), 'ocr-card-busy': operation?.mode === model.mode }"
        :aria-labelledby="`ocr-title-${model.mode}`"
        :aria-busy="operation?.mode === model.mode"
      >
        <div
          v-if="operation?.mode === model.mode && operation.kind !== 'switch'"
          class="ocr-progress-fill" :style="{ transform: `scaleX(${percent / 100})` }" aria-hidden="true"
        />
        <div class="ocr-card-body">
          <div class="ocr-card-heading">
            <h3 :id="`ocr-title-${model.mode}`">{{ t(`ocr.mode.${model.mode}.title`) }}</h3>
            <span class="ocr-size">{{ formatOcrBytes(statuses[model.mode]?.totalBytes ?? model.totalBytes) }}</span>
          </div>
          <p class="ocr-model-name">{{ model.name }}</p>
          <p class="ocr-description">{{ t(`ocr.mode.${model.mode}.description`) }}</p>
        </div>

        <div class="ocr-card-footer">
          <div
            v-if="operation?.mode === model.mode && operation.kind !== 'switch'"
            class="ocr-progress"
            role="progressbar"
            :aria-label="t(`ocr.mode.${model.mode}.title`)"
            :aria-valuenow="percent"
            :aria-valuetext="phaseText"
            aria-valuemin="0"
            aria-valuemax="100"
          >
            <div class="ocr-progress-copy">
              <span>{{ phaseText }}</span><strong>{{ percent }}%</strong>
              <span class="ocr-progress-detail">{{ formatOcrBytes(progress?.downloadedBytes ?? 0) }} / {{ formatOcrBytes(progress?.totalBytes ?? model.totalBytes) }}</span>
              <span class="ocr-progress-detail">{{ progress?.phase === 'downloading' ? `${formatOcrBytes(bytesPerSecond)}/s` : '' }}</span>
            </div>
          </div>
          <div v-else class="ocr-card-actions">
            <span v-if="loading" class="ocr-state"><LoaderCircle :size="14" class="update-spin" aria-hidden="true" />{{ t("ocr.status.checking") }}</span>
            <template v-else>
              <span v-if="inUse(model.mode)" class="ocr-state ocr-state-active"><Check :size="14" aria-hidden="true" />{{ t("ocr.status.inUse") }}</span>
              <span v-else-if="installed(model.mode)" class="ocr-state">{{ t("ocr.status.downloadedBadge") }}</span>
              <button
                v-if="!inUse(model.mode) || failure?.mode === model.mode"
                type="button" class="settings-action-button ocr-model-action" :disabled="busy"
                @click="choose(model.mode)"
              >
                <LoaderCircle v-if="operation?.mode === model.mode" :size="15" class="update-spin" aria-hidden="true" />
                <Download v-else-if="!installed(model.mode) && !needsRepair(model.mode)" :size="15" aria-hidden="true" />
                {{ actionText(model.mode) }}
              </button>
            </template>
          </div>
        </div>
        <details v-if="failure?.mode === model.mode" class="ocr-error-details">
          <summary>{{ t(failure.kind === 'install' ? 'ocr.downloadFailed' : 'ocr.operationFailed') }}<ChevronDown :size="14" /></summary>
          <p>{{ failure.detail }}</p>
        </details>
      </article>
    </div>

    <div class="ocr-storage">
      <p class="ocr-intro">{{ t("ocr.modelIntro") }}</p>
      <p v-if="installDir" class="ocr-path">{{ t("ocr.directory", { path: installDir }) }}</p>
      <div class="ocr-resource-actions">
        <button type="button" class="ocr-source" @click="openSource">
          {{ t("ocr.source") }}<ExternalLink :size="13" aria-hidden="true" />
        </button>
        <div class="settings-action-row">
          <button type="button" class="settings-action-button" :disabled="!installDir || !hasModels" @click="openDirectory">
            <FolderOpen :size="16" aria-hidden="true" />{{ t("ocr.openDownloadDir") }}
          </button>
          <button
            type="button" class="settings-action-button settings-action-button-danger"
            :class="{ 'ocr-remove-confirm': confirmRemoval }"
            :disabled="busy || !hasModels" :aria-busy="removing"
            @click="handleRemoval"
            @mouseleave="confirmRemoval = false"
            @blur="confirmRemoval = false"
            @keydown.esc.stop="confirmRemoval = false"
          >
            <LoaderCircle v-if="removing" :size="16" class="update-spin" aria-hidden="true" />
            <Trash2 v-else :size="16" aria-hidden="true" />
            <span class="ocr-remove-label">
              <span class="ocr-remove-measure" aria-hidden="true">{{ t("ocr.deleteResources") }}</span>
              <span aria-live="polite">{{ removalText }}</span>
            </span>
          </button>
        </div>
      </div>
    </div>

    <div v-if="error" class="settings-message settings-message-error ocr-general-error" role="alert">
      <AlertCircle :size="16" aria-hidden="true" />
      <details><summary>{{ t("ocr.operationFailed") }}</summary><p>{{ error }}</p></details>
      <button type="button" class="settings-action-button" :disabled="busy" @click="load">{{ t("ocr.refresh") }}</button>
    </div>
    <p v-if="notice" class="settings-message" role="status"><CheckCircle2 :size="16" aria-hidden="true" />{{ t(notice === 'removed' ? 'ocr.removedMessage' : 'ocr.repairedMessage') }}</p>
  </section>
</template>

<style scoped>
.ocr-settings {
  color: #334155;
}

.ocr-heading {
  margin: 0;
  color: #0f172a;
  font-size: 0.875rem;
  font-weight: 600;
}

.ocr-cards {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.75rem;
}

.ocr-card {
  position: relative;
  isolation: isolate;
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 1rem;
  overflow: hidden;
  border: 1px solid #e2e8f0;
  border-radius: 0.75rem;
  background: #f8fafc;
  padding: 1rem;
  transition: border-color 160ms ease, background-color 160ms ease;
}

.ocr-card-active {
  border-color: #0d9488;
  background: #f0fdfa;
}

.ocr-card-busy {
  border-color: #99f6e4;
  background: #ffffff;
}

.ocr-card-body,
.ocr-card-footer {
  position: relative;
}

.ocr-card-heading {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 0.375rem 0.75rem;
}

.ocr-card-heading h3 {
  margin: 0;
  color: #0f172a;
  font-size: 0.875rem;
  font-weight: 600;
}

.ocr-size,
.ocr-model-name {
  color: #64748b;
  font-size: 0.75rem;
  line-height: 1.5;
}

.ocr-size {
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}

.ocr-model-name {
  margin: 0.375rem 0 0;
}

.ocr-description {
  margin: 0.75rem 0 0;
  color: #64748b;
  font-size: 0.875rem;
  line-height: 1.6;
}

.ocr-card-footer {
  margin-top: auto;
}

.ocr-card-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.5rem;
  min-height: 2.25rem;
  justify-content: space-between;
}

.ocr-model-action {
  margin-left: auto;
}

.ocr-state {
  display: inline-flex;
  align-items: center;
  gap: 0.375rem;
  color: #64748b;
  font-size: 0.75rem;
}

.ocr-state-active {
  color: #0f766e;
  font-weight: 600;
}

.ocr-progress-fill {
  position: absolute;
  inset: 0;
  background: #ccfbf1;
  transform-origin: left;
  transition: transform 180ms linear;
  pointer-events: none;
}

.ocr-progress-copy {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-content: center;
  gap: 0.25rem 0.5rem;
  min-height: 2.25rem;
  color: #0f766e;
  font-size: 0.75rem;
  font-variant-numeric: tabular-nums;
}

.ocr-progress-copy strong {
  font-weight: 600;
  text-align: right;
}

.ocr-progress-detail {
  color: #64748b;
  overflow-wrap: anywhere;
}

.ocr-progress-detail:last-child {
  text-align: right;
}

.ocr-storage {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 0.75rem;
  border-top: 1px solid #edf1f5;
  padding-top: 1rem;
}

.ocr-intro,
.ocr-path {
  margin: 0;
  color: #64748b;
  line-height: 1.65;
}

.ocr-intro {
  font-size: 0.875rem;
}

.ocr-path {
  font-size: 0.75rem;
  overflow-wrap: anywhere;
  user-select: text;
}

.ocr-resource-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 0.75rem;
}

.ocr-source {
  display: inline-flex;
  align-items: center;
  gap: 0.375rem;
  min-height: 2rem;
  border: 0;
  background: transparent;
  padding: 0;
  color: #64748b;
  font-size: 0.75rem;
  text-align: left;
}

.ocr-error-details {
  position: relative;
  border-top: 1px solid #fecaca;
  padding-top: 0.75rem;
  color: #b91c1c;
  font-size: 0.75rem;
}

.ocr-error-details summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  list-style: none;
}

.ocr-error-details summary::-webkit-details-marker {
  display: none;
}

.ocr-error-details[open] summary svg {
  transform: rotate(180deg);
}

.ocr-error-details p,
.ocr-general-error p {
  margin: 0.5rem 0 0;
  line-height: 1.6;
  overflow-wrap: anywhere;
}

.ocr-remove-confirm {
  border-color: #fecaca;
  background: #fef2f2;
  color: #b91c1c;
}

.ocr-remove-label {
  display: grid;
}

.ocr-remove-label > span {
  grid-area: 1 / 1;
}

.ocr-remove-measure {
  visibility: hidden;
}

.ocr-general-error {
  align-items: flex-start;
  flex-wrap: wrap;
}

.ocr-general-error details {
  min-width: 0;
  flex: 1 1 12rem;
}

.ocr-settings svg {
  flex-shrink: 0;
}

.ocr-settings button:disabled {
  cursor: not-allowed;
  opacity: 0.56;
}

summary {
  cursor: pointer;
}

summary:focus-visible {
  outline: 2px solid #0d9488;
  outline-offset: 2px;
}

@media (hover: hover) and (pointer: fine) {
  .ocr-source:hover {
    color: #0f766e;
    text-decoration: underline;
    text-underline-offset: 3px;
  }
}

@media (max-width: 640px) {
  .ocr-cards {
    grid-template-columns: minmax(0, 1fr);
  }
}

@media (prefers-reduced-motion: reduce) {
  .ocr-card,
  .ocr-progress-fill {
    transition: none;
  }
}
</style>
