<script setup lang="ts">
import { computed } from "vue";
import { AlertCircle, CheckCircle2, Download, RotateCw, X } from "lucide-vue-next";
import { cleanUpdateNotes, type UpdateErrorPhase, type UpdateStatus } from "../composables/useUpdater";
import { t } from "../i18n";

type UpdateDialogInfo = {
  currentVersion: string;
  version: string;
  body?: string;
};

const props = defineProps<{
  open: boolean;
  status: UpdateStatus;
  update: UpdateDialogInfo | null;
  currentVersion?: string;
  error: string | null;
  errorPhase: UpdateErrorPhase;
  downloadedBytes: number;
  totalBytes: number | null;
}>();

const emit = defineEmits<{
  dismiss: [];
  install: [];
  relaunch: [];
}>();

const title = computed(() => {
  if (props.status === "downloading") return t("update.title.downloading");
  if (props.status === "ready") return t("update.title.ready");
  if (props.status === "error" && props.errorPhase === "install") return t("update.title.installError");
  if (props.status === "error" && props.errorPhase === "relaunch") return t("update.title.relaunchError");
  if (props.status === "error") return t("update.title.checkError");
  return t("update.title.available");
});

const releaseNotes = computed(() => cleanUpdateNotes(props.update?.body));

const progressPercent = computed(() => {
  if (!props.totalBytes || props.totalBytes <= 0) return 0;
  return Math.min(100, Math.max(0, Math.round((props.downloadedBytes / props.totalBytes) * 100)));
});

const progressText = computed(() => {
  if (!props.totalBytes && props.downloadedBytes <= 0) return t("update.progressPreparing");
  if (!props.totalBytes) return t("update.progressDownloaded", { bytes: formatBytes(props.downloadedBytes) });
  return `${formatBytes(props.downloadedBytes)} / ${formatBytes(props.totalBytes)}`;
});

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  return `${value >= 10 ? value.toFixed(0) : value.toFixed(1)} ${units[unitIndex]}`;
}
</script>

<template>
  <div v-if="open" class="update-dialog-backdrop" @click.self="emit('dismiss')">
    <section class="update-dialog" role="alertdialog" aria-modal="true" aria-labelledby="update-dialog-title">
      <header class="update-dialog-header">
        <div class="update-dialog-icon" :class="{ 'update-dialog-icon-error': status === 'error' }">
          <AlertCircle v-if="status === 'error'" class="size-5" />
          <CheckCircle2 v-else-if="status === 'ready'" class="size-5" />
          <Download v-else class="size-5" />
        </div>
        <div class="min-w-0">
          <h2 id="update-dialog-title">{{ title }}</h2>
          <p v-if="update">
            {{ t("update.versionLine", { current: currentVersion ?? update.currentVersion, next: update.version }) }}
          </p>
        </div>
        <button
          type="button"
          class="update-dialog-close"
          :disabled="status === 'downloading'"
          tabindex="-1"
          :aria-label="t('update.closePrompt')"
          :data-tooltip="t('update.closePrompt')"
          @click="emit('dismiss')"
        >
          <X class="size-4" />
        </button>
      </header>

      <div v-if="status === 'available'" class="update-dialog-body">
        <p>
          {{ t("update.availableBody") }}
        </p>
        <div v-if="releaseNotes" class="update-release-notes">
          {{ releaseNotes }}
        </div>
      </div>

      <div v-else-if="status === 'downloading'" class="update-dialog-body">
        <p>{{ t("update.downloadingBody") }}</p>
        <div class="update-progress">
          <div class="update-progress-bar">
            <span :style="{ width: `${progressPercent}%` }" />
          </div>
          <span>{{ progressText }}</span>
        </div>
      </div>

      <div v-else-if="status === 'ready'" class="update-dialog-body">
        <p>{{ t("update.readyBody") }}</p>
      </div>

      <div v-else-if="status === 'error'" class="update-dialog-body">
        <p>{{ error }}</p>
      </div>

      <footer class="update-dialog-actions">
        <button
          v-if="status === 'available'"
          type="button"
          class="settings-action-button"
          tabindex="-1"
          @click="emit('dismiss')"
        >
          <span>{{ t("common.later") }}</span>
        </button>
        <button
          v-if="status === 'available'"
          type="button"
          class="settings-action-button settings-action-button-primary"
          tabindex="-1"
          @click="emit('install')"
        >
          <Download class="size-4" />
          <span>{{ t("update.installNow") }}</span>
        </button>
        <button
          v-else-if="status === 'ready'"
          type="button"
          class="settings-action-button settings-action-button-primary"
          tabindex="-1"
          @click="emit('relaunch')"
        >
          <RotateCw class="size-4" />
          <span>{{ t("update.restartNow") }}</span>
        </button>
        <button
          v-else-if="status === 'error'"
          type="button"
          class="settings-action-button"
          tabindex="-1"
          @click="emit('dismiss')"
        >
          <span>{{ t("common.gotIt") }}</span>
        </button>
      </footer>
    </section>
  </div>
</template>
