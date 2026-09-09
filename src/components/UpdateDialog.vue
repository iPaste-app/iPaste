<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { Download, ExternalLink, RotateCw, X } from "lucide-vue-next";
import { cleanUpdateNotes, type UpdateErrorPhase, type UpdateStatus } from "../composables/useUpdater";
import { t } from "../i18n";
import { openGitHubRelease } from "../lib/starPrompt";
import MarkdownPreview from "./MarkdownPreview.vue";

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

const dialogElement = ref<HTMLElement | null>(null);
let previousFocus: HTMLElement | null = null;

watch(() => props.open, async (open) => {
  if (open) {
    previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    await nextTick();
    if (props.open) dialogElement.value?.focus({ preventScroll: true });
  } else if (previousFocus?.isConnected) {
    previousFocus.focus({ preventScroll: true });
  }
}, { immediate: true });

const title = computed(() => {
  if (props.status === "downloading") return t("update.title.downloading");
  if (props.status === "ready") return t("update.title.ready");
  if (props.status === "error" && props.errorPhase === "install") return t("update.title.installError");
  if (props.status === "error" && props.errorPhase === "relaunch") return t("update.title.relaunchError");
  if (props.status === "error") return t("update.title.checkError");
  return t("update.title.available");
});

const releaseNotes = computed(() => cleanUpdateNotes(props.update?.body));
const releaseNotesLinkText = computed(() =>
  t(releaseNotes.value ? "update.viewFullReleaseNotes" : "update.viewReleaseNotes"),
);

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

async function openReleaseNotes() {
  if (!props.update?.version) return;

  try {
    await openGitHubRelease(props.update.version);
  } catch (error) {
    console.warn("[ipaste] failed to open GitHub release", error);
  }
}
</script>

<template>
  <div v-if="open" class="update-dialog-backdrop" @click.self="emit('dismiss')">
    <section
      ref="dialogElement"
      class="update-dialog"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="update-dialog-title"
      tabindex="-1"
      @keydown.esc.stop.prevent="emit('dismiss')"
    >
      <header class="update-dialog-header">
        <h2 id="update-dialog-title">
          <span>{{ title }}</span>
          <span v-if="update" class="update-dialog-version">
            {{ currentVersion ?? update.currentVersion }} → {{ update.version }}
          </span>
        </h2>
        <button
          type="button"
          class="update-dialog-close"
          :aria-label="t('update.closePrompt')"
          :data-tooltip="t('update.closePrompt')"
          @click="emit('dismiss')"
        >
          <X class="size-4" />
        </button>
      </header>

      <div v-if="status === 'available'" class="update-dialog-body update-dialog-notes-body">
        <div class="update-release-panel">
          <MarkdownPreview v-if="releaseNotes" class="update-release-notes" :source="releaseNotes" />
          <button type="button" class="update-release-link" @click="openReleaseNotes">
            <span>{{ releaseNotesLinkText }}</span>
            <ExternalLink class="size-3.5" aria-hidden="true" />
          </button>
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
          v-if="status === 'downloading'"
          type="button"
          class="settings-action-button"
          @click="emit('dismiss')"
        >
          <span>{{ t("update.downloadInBackground") }}</span>
        </button>
        <button
          v-if="status === 'available'"
          type="button"
          class="settings-action-button"
          @click="emit('dismiss')"
        >
          <span>{{ t("common.later") }}</span>
        </button>
        <button
          v-if="status === 'available'"
          type="button"
          class="settings-action-button settings-action-button-primary"
          @click="emit('install')"
        >
          <Download class="size-4" />
          <span>{{ t("update.installNow") }}</span>
        </button>
        <button
          v-else-if="status === 'ready'"
          type="button"
          class="settings-action-button settings-action-button-primary"
          @click="emit('relaunch')"
        >
          <RotateCw class="size-4" />
          <span>{{ t("update.restartNow") }}</span>
        </button>
        <button
          v-else-if="status === 'error'"
          type="button"
          class="settings-action-button"
          @click="emit('dismiss')"
        >
          <span>{{ t("common.gotIt") }}</span>
        </button>
      </footer>
    </section>
  </div>
</template>
