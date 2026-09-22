<script setup lang="ts">
import { computed } from "vue";
import { t } from "../i18n";
import type { MfaAccount } from "../types";
import { secondsRemaining } from "../lib/mfa";

const props = defineProps<{
  account: MfaAccount;
  code: string;
  nowMs: number;
  selected: boolean;
  busy: boolean;
  invalid: boolean;
  menuOpen: boolean;
}>();

const emit = defineEmits<{
  select: [];
  paste: [];
  contextMenu: [event: MouseEvent];
}>();

const remaining = computed(() => secondsRemaining(props.account.period, props.nowMs));
const progress = computed(() => remaining.value / props.account.period);
const countdownStage = computed(() => {
  if (progress.value <= 1 / 3) return "danger";
  if (progress.value <= 2 / 3) return "warning";
  return "safe";
});

function selectRow(event: MouseEvent) {
  emit("select");
  (event.currentTarget as HTMLElement).focus({ preventScroll: true });
}

function pasteRow() {
  if (!props.busy && !props.invalid && props.code) emit("paste");
}
</script>

<template>
  <article
    class="mfa-account-row"
    :class="{ 'is-selected': selected, 'is-menu-open': menuOpen }"
    :tabindex="selected ? 0 : -1"
    :aria-label="[account.issuer, account.name].filter(Boolean).join(' · ')"
    aria-keyshortcuts="ArrowUp ArrowDown ArrowLeft ArrowRight Enter Control+c Meta+c"
    :data-account-id="account.id"
    @click="selectRow"
    @focusin="emit('select')"
    @dblclick.prevent="pasteRow"
    @contextmenu.prevent.stop="emit('contextMenu', $event)"
  >
    <div class="mfa-account-identity">
      <strong>{{ account.issuer || account.name }}</strong>
      <span v-if="account.issuer && account.issuer !== account.name" class="mfa-account-name">{{ account.name }}</span>
      <p v-if="account.description">{{ account.description }}</p>
    </div>
    <div class="mfa-account-code" :class="{ 'mfa-account-code-long': account.digits > 6 }">
      <span v-if="invalid" class="mfa-code-error" role="status">{{ t('apps.mfa.codeError') }}</span>
      <template v-else>
        <span
          class="mfa-code-value"
          dir="ltr"
        >{{ code || "—".repeat(account.digits) }}</span>
        <span
          v-if="code"
          class="mfa-code-countdown"
          :class="`is-${countdownStage}`"
          role="img"
          :aria-label="t('apps.mfa.expiresIn', { seconds: remaining })"
        >
          <svg viewBox="0 0 36 36" aria-hidden="true">
            <circle class="mfa-countdown-track" cx="18" cy="18" r="15" />
            <circle
              class="mfa-countdown-progress"
              cx="18" cy="18" r="15"
              pathLength="100"
              stroke-dasharray="100"
              :stroke-dashoffset="(1 - progress) * 100"
            />
          </svg>
          <span aria-hidden="true">{{ remaining }}s</span>
        </span>
      </template>
    </div>
  </article>
</template>

<style scoped>
.mfa-account-row {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  min-width: 0;
  min-height: 8.75rem;
  border: 1px solid #e2e8f0;
  border-radius: 0.75rem;
  background: rgba(255, 255, 255, 0.92);
  padding: 0.625rem;
  transition: border-color 160ms ease, background-color 160ms ease, box-shadow 160ms ease;
}

.mfa-account-row:hover {
  border-color: #cbd5e1;
  background: #f8fafc;
}

.mfa-account-row.is-selected,
.mfa-account-row.is-menu-open {
  border-color: #2563eb;
  background: #fff;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.2), 0 10px 24px rgba(37, 99, 235, 0.12);
}

.mfa-account-row:focus-visible {
  outline: 2px solid #2563eb;
  outline-offset: 2px;
}

.mfa-account-identity {
  display: grid;
  gap: 0.1875rem;
  min-width: 0;
  overflow-wrap: anywhere;
}

.mfa-account-identity strong {
  color: #0f172a;
  font-size: 0.875rem;
  font-weight: 700;
  line-height: 1.45;
}

.mfa-account-name {
  color: #475569;
  font-size: 0.8125rem;
  line-height: 1.5;
}

.mfa-account-identity p {
  margin: 0.125rem 0 0;
  color: #64748b;
  font-size: 0.75rem;
  line-height: 1.5;
  white-space: pre-wrap;
}

.mfa-account-code {
  --mfa-code-font-size: clamp(1.25rem, 13cqi, 2rem);
  container-type: inline-size;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.25rem;
  margin-top: auto;
  padding-block: 0.125rem;
}

.mfa-account-code-long {
  --mfa-code-font-size: clamp(1rem, 10cqi, 1.75rem);
}

.mfa-code-value {
  display: block;
  border-radius: 0.375rem;
  padding: 0.375rem;
  background: #eff6ff;
  color: #1d4ed8;
  font-family: "SFMono-Regular", Consolas, "Liberation Mono", monospace;
  font-size: var(--mfa-code-font-size);
  font-weight: 800;
  line-height: 1.1;
  letter-spacing: 0.035em;
  font-variant-numeric: tabular-nums slashed-zero;
  white-space: nowrap;
}

.mfa-code-countdown {
  --countdown-color: #15803d;
  --countdown-track: #dcfce7;
  position: relative;
  display: grid;
  width: calc(var(--mfa-code-font-size) * 1.1 + 0.75rem);
  height: calc(var(--mfa-code-font-size) * 1.1 + 0.75rem);
  place-items: center;
  flex-shrink: 0;
  color: var(--countdown-color);
  font-size: clamp(0.625rem, calc(var(--mfa-code-font-size) * 0.4), 0.8125rem);
  font-weight: 650;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.mfa-code-countdown svg {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  fill: none;
  stroke-width: 2;
  transform: rotate(-90deg);
}

.mfa-countdown-track {
  stroke: var(--countdown-track);
}

.mfa-countdown-progress {
  stroke: currentColor;
  stroke-linecap: round;
}

.mfa-code-countdown.is-warning {
  --countdown-color: #a16207;
  --countdown-track: #fef9c3;
}

.mfa-code-countdown.is-danger {
  --countdown-color: #dc2626;
  --countdown-track: #fee2e2;
}

.mfa-code-error {
  color: #b91c1c;
  font-size: 0.75rem;
}

@media (prefers-reduced-motion: reduce) {
  .mfa-account-row { transition: none; }
}
</style>
