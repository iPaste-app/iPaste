import { computed, onMounted, onUnmounted, ref } from "vue";
import { lanShareApi, shareCountdown, shareError, type LanShare } from "../lib/lanShare";
import { ipasteApi } from "../lib/ipasteApi";
import { t } from "../i18n";
import type { ClipViewItem } from "../types";

export function useLanShare(item: ClipViewItem) {
  const share = ref<LanShare | null>(null);
  const busy = ref(false);
  const error = ref("");
  const copied = ref(false);
  const replaced = ref(false);
  const now = ref(Date.now());
  const active = computed(() => share.value !== null && now.value < share.value.expiresAt);
  const countdown = computed(() => share.value ? shareCountdown(share.value.expiresAt, now.value) : "0:00");
  const qrSource = computed(() => share.value ? `data:image/svg+xml;charset=utf-8,${encodeURIComponent(share.value.qrSvg)}` : "");
  let disposed = false;
  let checking = false;
  let interval: ReturnType<typeof setInterval> | undefined;
  let feedbackTimer: ReturnType<typeof setTimeout> | undefined;

  async function start() {
    if (busy.value) return;
    busy.value = true;
    error.value = "";
    try {
      const result = await lanShareApi.start(item);
      if (disposed) return;
      now.value = Date.now();
      share.value = result;
      replaced.value = result.replaced;
    } catch (cause) {
      if (!disposed) error.value = shareError(cause);
    } finally {
      if (!disposed) busy.value = false;
    }
  }

  async function stop() {
    if (!share.value || busy.value) return;
    busy.value = true;
    error.value = "";
    try {
      await lanShareApi.stop(share.value.id);
      if (!disposed) share.value = null;
    } catch {
      if (!disposed) error.value = t("share.error.stop");
    } finally {
      if (!disposed) busy.value = false;
    }
  }

  async function copyLink() {
    now.value = Date.now();
    if (!active.value || !share.value) return;
    error.value = "";
    try {
      // Sharing URLs should not create another clipboard-history entry.
      await ipasteApi.copyTextEphemeral(share.value.url);
      if (disposed) return;
      copied.value = true;
      clearTimeout(feedbackTimer);
      feedbackTimer = setTimeout(() => { copied.value = false; }, 1800);
    } catch {
      if (!disposed) error.value = t("share.error.copy");
    }
  }

  async function refresh() {
    now.value = Date.now();
    if (!share.value || busy.value || checking || disposed) return;
    checking = true;
    const id = share.value.id;
    try {
      const current = await lanShareApi.current();
      if (!disposed && share.value?.id === id && current?.id !== id) share.value = null;
    } catch { /* The native timer still owns expiration when status cannot be read. */ }
    finally { checking = false; }
  }

  onMounted(() => {
    void start();
    interval = setInterval(() => { void refresh(); }, 1000);
  });
  onUnmounted(() => {
    disposed = true;
    clearInterval(interval);
    clearTimeout(feedbackTimer);
    // Closing the dialog intentionally keeps the native share alive.
  });

  return { share, busy, error, copied, replaced, active, countdown, qrSource, start, stop, copyLink };
}
