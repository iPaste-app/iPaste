<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref } from "vue";
import { Check, Copy, X } from "lucide-vue-next";
import { useLanShare } from "../composables/useLanShare";
import { t } from "../i18n";
import type { ClipViewItem } from "../types";

const props = defineProps<{ item: ClipViewItem }>();
const emit = defineEmits<{ dismiss: []; ended: [] }>();
const dialog = ref<HTMLElement | null>(null);
const { share, busy, error, copied, active, countdown, qrSource, start, stop, copyLink } = useLanShare(props.item);
const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;

onMounted(async () => {
  await nextTick();
  dialog.value?.focus({ preventScroll: true });
});
onUnmounted(() => {
  if (previousFocus?.isConnected) previousFocus.focus({ preventScroll: true });
});

function trapFocus(event: KeyboardEvent) {
  if (event.key !== "Tab") return;
  const controls = dialog.value?.querySelectorAll<HTMLElement>('button:not(:disabled), [tabindex="0"]');
  if (!controls?.length) return;
  const first = controls[0];
  const last = controls[controls.length - 1];
  if (event.shiftKey && (document.activeElement === first || document.activeElement === dialog.value)) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && (document.activeElement === last || document.activeElement === dialog.value)) {
    event.preventDefault();
    first.focus();
  }
}

function selectLink(event: FocusEvent) {
  const target = event.currentTarget;
  const selection = window.getSelection();
  if (!(target instanceof HTMLElement) || !selection) return;
  const range = document.createRange();
  range.selectNodeContents(target);
  selection.removeAllRanges();
  selection.addRange(range);
}

async function endShare() {
  if (!share.value || busy.value) return;
  await stop();
  if (!share.value && !error.value) emit("ended");
}

defineExpose({ handleNativeEnter: () => { if (active.value && !busy.value) void copyLink(); } });
</script>

<template>
  <div class="share-backdrop" @click.stop @click.self="emit('dismiss')" @contextmenu.prevent>
    <section
      ref="dialog"
      class="share-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="share-title"
      tabindex="-1"
      @keydown="trapFocus"
      @keydown.esc.stop.prevent="emit('dismiss')"
    >
      <header>
        <h2 id="share-title">{{ t('share.title') }}</h2>
        <button class="share-close" type="button" :aria-label="t('common.close')" @click="emit('dismiss')">
          <X :size="18" aria-hidden="true" />
        </button>
      </header>

      <div class="share-body">
        <template v-if="active && share">
          <img class="share-qr" :src="qrSource" :alt="t('share.qr')" width="224" height="224" draggable="false">
          <p class="share-hint">{{ t('share.networkHint') }}</p>
          <div class="share-link-group">
            <div
              class="share-link"
              role="textbox"
              aria-readonly="true"
              aria-multiline="true"
              :aria-label="t('share.link')"
              tabindex="0"
              @focus="selectLink"
            >{{ share.url }}</div>
            <button class="share-copy" type="button" :disabled="busy" :aria-label="t('share.copyLink')" @click="copyLink">
              <Check v-if="copied" :size="15" aria-hidden="true" />
              <Copy v-else :size="15" aria-hidden="true" />
              <span aria-live="polite">{{ t(copied ? 'share.copied' : 'share.copyLink') }}</span>
            </button>
          </div>
          <p class="share-countdown">{{ t('share.remaining', { time: countdown }) }}</p>
        </template>
        <p v-else-if="busy" class="share-state" role="status">{{ t('share.preparing') }}</p>
        <p v-else-if="!error" class="share-state" role="status">{{ t('share.ended') }}</p>

        <p v-if="error" class="share-error" role="alert">{{ error }}</p>
      </div>

      <footer>
        <template v-if="active">
          <p>{{ t('share.keepAlive') }}</p>
          <button class="share-stop" type="button" :disabled="busy" @click="endShare">{{ t('share.stop') }}</button>
        </template>
        <button v-else-if="error" class="share-copy" type="button" :disabled="busy" @click="start">{{ t('share.retry') }}</button>
        <button v-else class="share-secondary" type="button" @click="emit('dismiss')">{{ t('common.close') }}</button>
      </footer>
    </section>
  </div>
</template>

<style scoped>
.share-backdrop {
  position: fixed;
  inset: 0;
  z-index: 96;
  display: grid;
  place-items: center;
  padding: 12px;
  background: rgb(15 23 42 / 30%);
  backdrop-filter: blur(6px);
}
.share-dialog {
  width: min(100%, 372px);
  max-height: calc(100dvh - 24px);
  overflow-y: auto;
  border: 1px solid #dbe1e9;
  border-radius: 14px;
  background: #fff;
  box-shadow: 0 18px 48px rgb(15 23 42 / 18%);
  outline: none;
}
header { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 14px 16px 0; }
h2 { margin: 0; font-size: 15px; font-weight: 650; }
button { display: inline-flex; align-items: center; justify-content: center; gap: 6px; border: 0; border-radius: 7px; font-size: 12px; line-height: 1.4; }
button:active:not(:disabled) { transform: scale(0.98); }
button:disabled { opacity: 0.5; cursor: default; }
.share-close { width: 30px; height: 30px; flex: 0 0 auto; color: #64748b; background: transparent; }
.share-close:hover { background: #f1f5f9; }
.share-body { padding: 10px 16px 16px; text-align: center; }
.share-qr { display: block; width: min(100%, 224px); height: auto; aspect-ratio: 1; margin: 0 auto 8px; background: #fff; }
.share-hint, .share-countdown { margin: 8px 0 0; color: #64748b; font-size: 12px; line-height: 1.5; }
.share-link-group { display: flex; flex-direction: column; gap: 8px; margin-top: 14px; }
.share-link { min-width: 0; padding: 8px; color: #475569; border: 1px solid #e2e8f0; border-radius: 7px; background: #f8fafc; font-size: 12px; line-height: 1.6; text-align: left; overflow-wrap: anywhere; user-select: text; }
.share-link:focus-visible { outline: 2px solid var(--theme-accent); outline-offset: 1px; }
.share-copy { padding: 8px 10px; color: #fff; background: var(--theme-accent); }
.share-copy:hover { background: var(--theme-accent-hover); }
.share-countdown { font-variant-numeric: tabular-nums; }
.share-state { margin: 32px 0; color: #64748b; font-size: 14px; }
.share-error { margin: 14px 0 0; color: #b91c1c; font-size: 12px; line-height: 1.6; }
footer { display: flex; align-items: center; justify-content: flex-end; gap: 12px; padding: 12px 16px; border-top: 1px solid #f1f5f9; }
footer p { flex: 1; margin: 0; color: #94a3b8; font-size: 11px; line-height: 1.5; }
.share-stop, .share-secondary { flex: 0 0 auto; padding: 7px 9px; color: #475569; background: #f1f5f9; }
.share-stop:hover { color: #b91c1c; background: #fef2f2; }
</style>
