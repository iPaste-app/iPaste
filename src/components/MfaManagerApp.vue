<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, reactive, ref, watch } from "vue";
import {
  Check,
  ChevronLeft,
  ClipboardCopy,
  CornerDownLeft,
  History,
  KeyRound,
  MoreHorizontal,
  Pencil,
  Plus,
  QrCode,
  RefreshCw,
  ShieldCheck,
  SlidersHorizontal,
  Trash2,
  Upload,
  X,
} from "lucide-vue-next";
import { t } from "../i18n";
import { ipasteApi } from "../lib/ipasteApi";
import {
  generateTotp,
  isValidMfaSecret,
  mfaInputFromAccount,
  normalizeMfaAlgorithm,
  normalizeMfaDigits,
  normalizeMfaPeriod,
  normalizeMfaSecret,
  parseOtpAuthUri,
  secondsRemaining,
} from "../lib/mfa";
import { decodeQrFromClip, decodeQrFromFile } from "../lib/qr";
import { useIpasteStore } from "../stores/ipasteStore";
import type { ClipItem, MfaAccount, MfaAccountInput, MfaAlgorithm } from "../types";

const emit = defineEmits<{
  back: [];
}>();

type HistoryScanLimit = 10 | 20 | 30 | 50;
type CandidateSource = "clipboard" | "history" | "upload";

type MfaDraft = {
  name: string;
  issuer: string;
  description: string;
  secret: string;
  algorithm: MfaAlgorithm;
  digits: number;
  period: number;
  sourceUri: string;
};

type MfaCandidate = {
  id: string;
  input: MfaAccountInput;
  source: CandidateSource;
  sourceLabel: string;
  previewText?: string;
};

type AccountContextMenu = {
  account: MfaAccount;
  x: number;
  y: number;
};

const store = useIpasteStore();
const accounts = ref<MfaAccount[]>([]);
const isLoading = ref(false);
const isSaving = ref(false);
const error = ref("");
const notice = ref("");
const formOpen = ref(false);
const editingId = ref<string | null>(null);
const draft = reactive<MfaDraft>(emptyDraft());
const pendingCandidate = ref<MfaCandidate | null>(null);
const ignoredCandidateKeys = ref<Set<string>>(new Set());
const historyScanLimit = ref<HistoryScanLimit>(10);
const historyCandidates = ref<MfaCandidate[]>([]);
const isScanningClipboard = ref(false);
const isScanningHistory = ref(false);
const isScanningUpload = ref(false);
const uploadInput = ref<HTMLInputElement | null>(null);
const codeById = ref<Record<string, string>>({});
const codeErrorById = ref<Record<string, string>>({});
const nowMs = ref(Date.now());
const pendingDeleteId = ref<string | null>(null);
const copiedAccountId = ref<string | null>(null);
const accountContextMenu = ref<AccountContextMenu | null>(null);
const accountContextMenuElement = ref<HTMLElement | null>(null);
let accountContextMenuReturnFocus: HTMLElement | null = null;
let codeTimer: number | null = null;
let codeRefreshId = 0;
let copiedTimer: number | null = null;
let noticeTimer: number | null = null;
let errorTimer: number | null = null;

const scanLimitOptions: HistoryScanLimit[] = [10, 20, 30, 50];
const algorithmOptions: MfaAlgorithm[] = ["SHA1", "SHA256", "SHA512"];
const accountCountLabel = computed(() => t("apps.mfa.accountCount", { count: accounts.value.length }));
const parsedDraftUri = computed(() => parseOtpAuthUri(draft.secret) ?? parseOtpAuthUri(draft.sourceUri));
const saveDisabled = computed(() =>
  isSaving.value || !(draft.name.trim() || parsedDraftUri.value?.name) || !isValidMfaSecret(normalizedDraftSecret.value),
);
const normalizedDraftSecret = computed(() => {
  return parsedDraftUri.value ? parsedDraftUri.value.secret : normalizeMfaSecret(draft.secret);
});

watch(notice, (message) => {
  if (noticeTimer !== null) window.clearTimeout(noticeTimer);
  if (!message) return;
  noticeTimer = window.setTimeout(() => {
    if (notice.value === message) notice.value = "";
  }, 2800);
});

watch(error, (message) => {
  if (errorTimer !== null) window.clearTimeout(errorTimer);
  if (!message) return;
  errorTimer = window.setTimeout(() => {
    if (error.value === message) error.value = "";
  }, 5200);
});

onMounted(async () => {
  document.addEventListener("keydown", handleDocumentKeydown, true);
  window.addEventListener("blur", dismissAccountContextMenu);
  await loadAccounts();
  startCodeTimer();
  void scanLatestClipboard({ silent: true });
});

onUnmounted(() => {
  document.removeEventListener("keydown", handleDocumentKeydown, true);
  window.removeEventListener("blur", dismissAccountContextMenu);
  if (codeTimer !== null) {
    window.clearInterval(codeTimer);
    codeTimer = null;
  }
  if (copiedTimer !== null) window.clearTimeout(copiedTimer);
  if (noticeTimer !== null) window.clearTimeout(noticeTimer);
  if (errorTimer !== null) window.clearTimeout(errorTimer);
});

watch(
  () => store.clips[0]?.id,
  () => {
    void scanLatestClipboard({ silent: true });
  },
);

async function loadAccounts() {
  isLoading.value = true;
  error.value = "";
  try {
    accounts.value = await ipasteApi.listMfaAccounts();
    await refreshCodes();
  } catch (unknownError) {
    error.value = String(unknownError);
  } finally {
    isLoading.value = false;
  }
}

function startCodeTimer() {
  nowMs.value = Date.now();
  void refreshCodes();
  codeTimer = window.setInterval(() => {
    nowMs.value = Date.now();
    void refreshCodes();
  }, 1000);
}

async function refreshCodes() {
  const refreshId = ++codeRefreshId;
  const nextCodes: Record<string, string> = {};
  const nextErrors: Record<string, string> = {};
  await Promise.all(accounts.value.map(async (account) => {
    try {
      nextCodes[account.id] = await generateTotp(account, nowMs.value);
    } catch (unknownError) {
      nextErrors[account.id] = String(unknownError);
    }
  }));
  if (refreshId !== codeRefreshId) return;
  codeById.value = nextCodes;
  codeErrorById.value = nextErrors;
}

function remainingFor(account: MfaAccount) {
  return secondsRemaining(account.period, nowMs.value);
}

function progressFor(account: MfaAccount) {
  return Math.max(0, Math.min(1, remainingFor(account) / account.period));
}

function progressPercentFor(account: MfaAccount) {
  return `${Math.round(progressFor(account) * 100)}%`;
}

function timerStageFor(account: MfaAccount) {
  const ratio = progressFor(account);
  if (ratio <= 1 / 3) return "danger";
  if (ratio <= 2 / 3) return "warning";
  return "safe";
}

function openAccountContextMenu(account: MfaAccount, event: MouseEvent) {
  pendingDeleteId.value = null;
  accountContextMenuReturnFocus = event.currentTarget instanceof HTMLElement ? event.currentTarget : null;
  accountContextMenu.value = {
    account,
    x: event.clientX,
    y: event.clientY,
  };
  void nextTick(positionAccountContextMenu);
}

function toggleAccountContextMenu(account: MfaAccount, event: MouseEvent) {
  if (accountContextMenu.value?.account.id === account.id) {
    closeAccountContextMenu();
    return;
  }

  const trigger = event.currentTarget as HTMLElement;
  const rect = trigger.getBoundingClientRect();
  pendingDeleteId.value = null;
  accountContextMenuReturnFocus = trigger;
  accountContextMenu.value = {
    account,
    x: rect.right - 160,
    y: rect.bottom + 4,
  };
  void nextTick(() => {
    positionAccountContextMenu();
    accountContextMenuElement.value?.querySelector<HTMLElement>("[role='menuitem']")?.focus();
  });
}

function positionAccountContextMenu() {
  if (!accountContextMenu.value || !accountContextMenuElement.value) return;

  const rect = accountContextMenuElement.value.getBoundingClientRect();
  const padding = 8;
  const maxX = Math.max(padding, window.innerWidth - rect.width - padding);
  const maxY = Math.max(padding, window.innerHeight - rect.height - padding);
  accountContextMenu.value = {
    ...accountContextMenu.value,
    x: clamp(accountContextMenu.value.x, padding, maxX),
    y: clamp(accountContextMenu.value.y, padding, maxY),
  };
}

function closeAccountContextMenu(options: { restoreFocus?: boolean } = {}) {
  const returnFocus = accountContextMenuReturnFocus;
  accountContextMenu.value = null;
  pendingDeleteId.value = null;
  accountContextMenuReturnFocus = null;
  if (options.restoreFocus) void nextTick(() => returnFocus?.focus());
}

function dismissAccountContextMenu() {
  closeAccountContextMenu();
}

function handleDocumentKeydown(event: KeyboardEvent) {
  if (event.key !== "Escape") return;

  if (accountContextMenu.value) {
    event.preventDefault();
    event.stopPropagation();
    closeAccountContextMenu({ restoreFocus: true });
    return;
  }

  if (formOpen.value) {
    event.preventDefault();
    event.stopPropagation();
    cancelForm();
  }
}

function handleAccountMenuKeydown(event: KeyboardEvent) {
  if (!accountContextMenuElement.value) return;
  const items = [...accountContextMenuElement.value.querySelectorAll<HTMLElement>("[role='menuitem']")];
  if (!items.length) return;
  const currentIndex = items.indexOf(document.activeElement as HTMLElement);
  let nextIndex: number | null = null;

  if (event.key === "ArrowDown") nextIndex = (currentIndex + 1) % items.length;
  if (event.key === "ArrowUp") nextIndex = (currentIndex - 1 + items.length) % items.length;
  if (event.key === "Home") nextIndex = 0;
  if (event.key === "End") nextIndex = items.length - 1;
  if (nextIndex === null) return;

  event.preventDefault();
  items[nextIndex]?.focus();
}

async function copyAccountFromContext() {
  const account = accountContextMenu.value?.account;
  closeAccountContextMenu();
  if (!account) return;
  await copyAccountCode(account);
}

async function pasteAccountFromContext() {
  const account = accountContextMenu.value?.account;
  closeAccountContextMenu();
  if (!account) return;
  await pasteAccountCode(account);
}

function editAccountFromContext() {
  const account = accountContextMenu.value?.account;
  closeAccountContextMenu();
  if (!account) return;
  startEdit(account);
}

async function deleteAccountFromContext() {
  const account = accountContextMenu.value?.account;
  if (!account) return;

  if (pendingDeleteId.value !== account.id) {
    pendingDeleteId.value = account.id;
    return;
  }

  await deleteAccount(account);
  closeAccountContextMenu();
}

function accountDeleteLabel(account: MfaAccount) {
  return pendingDeleteId.value === account.id ? t("common.confirmDelete") : t("common.delete");
}

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

function startCreate() {
  editingId.value = null;
  Object.assign(draft, emptyDraft());
  formOpen.value = true;
  error.value = "";
  void scanLatestClipboard({ silent: false });
}

function startEdit(account: MfaAccount) {
  const input = mfaInputFromAccount(account);
  editingId.value = account.id;
  Object.assign(draft, {
    name: input.name,
    issuer: input.issuer ?? "",
    description: input.description ?? "",
    secret: input.secret,
    algorithm: input.algorithm ?? "SHA1",
    digits: input.digits ?? 6,
    period: input.period ?? 30,
    sourceUri: input.sourceUri ?? "",
  });
  formOpen.value = true;
  pendingDeleteId.value = null;
  error.value = "";
}

function cancelForm() {
  editingId.value = null;
  formOpen.value = false;
  Object.assign(draft, emptyDraft());
}

async function saveDraft() {
  const parsed = parseOtpAuthUri(draft.secret) ?? parseOtpAuthUri(draft.sourceUri);
  const input: MfaAccountInput = parsed
    ? {
        ...parsed,
        name: draft.name.trim() || parsed.name,
        issuer: draft.issuer.trim() || parsed.issuer || null,
        description: draft.description.trim() || parsed.description || null,
      }
    : {
        name: draft.name.trim(),
        issuer: draft.issuer.trim() || null,
        description: draft.description.trim() || null,
        secret: normalizeMfaSecret(draft.secret),
        algorithm: normalizeMfaAlgorithm(draft.algorithm),
        digits: normalizeMfaDigits(draft.digits),
        period: normalizeMfaPeriod(draft.period),
        sourceUri: draft.sourceUri.trim() || null,
      };

  if (!input.name.trim()) {
    error.value = t("apps.mfa.errorNameRequired");
    return;
  }
  if (!isValidMfaSecret(input.secret)) {
    error.value = t("apps.mfa.errorSecretInvalid");
    return;
  }

  isSaving.value = true;
  error.value = "";
  try {
    if (editingId.value) {
      const updated = await ipasteApi.updateMfaAccount(editingId.value, input);
      accounts.value = accounts.value.map((account) => (account.id === updated.id ? updated : account));
      notice.value = t("apps.mfa.updated");
    } else {
      const created = await ipasteApi.createMfaAccount(input);
      accounts.value = [created, ...accounts.value];
      notice.value = t("apps.mfa.created");
    }
    cancelForm();
    pendingCandidate.value = null;
    await refreshCodes();
  } catch (unknownError) {
    error.value = String(unknownError);
  } finally {
    isSaving.value = false;
  }
}

async function copyAccountCode(account: MfaAccount) {
  const code = await codeFor(account);
  await ipasteApi.copyTextEphemeral(code);
  await touchAccount(account.id);
  copiedAccountId.value = account.id;
  if (copiedTimer !== null) window.clearTimeout(copiedTimer);
  copiedTimer = window.setTimeout(() => {
    if (copiedAccountId.value === account.id) copiedAccountId.value = null;
  }, 1600);
  notice.value = t("apps.mfa.copied");
}

async function copyAccountCodeFromCard(account: MfaAccount) {
  closeAccountContextMenu();
  await copyAccountCode(account);
}

async function pasteAccountCode(account: MfaAccount) {
  const code = await codeFor(account);
  await ipasteApi.applyTextEphemeral(code);
  await touchAccount(account.id);
}

async function pasteAccountCodeFromCard(account: MfaAccount) {
  closeAccountContextMenu();
  await pasteAccountCode(account);
}

async function codeFor(account: MfaAccount) {
  // The displayed code may be from before a timer boundary or a suspended window.
  return generateTotp(account);
}

async function touchAccount(id: string) {
  try {
    const updated = await ipasteApi.touchMfaAccount(id);
    accounts.value = accounts.value.map((account) => (account.id === id ? updated : account));
  } catch {
    // Touch is best-effort; copy/paste already succeeded.
  }
}

async function deleteAccount(account: MfaAccount) {
  if (pendingDeleteId.value !== account.id) {
    pendingDeleteId.value = account.id;
    return;
  }

  pendingDeleteId.value = null;
  await ipasteApi.deleteMfaAccount(account.id);
  accounts.value = accounts.value.filter((item) => item.id !== account.id);
  if (editingId.value === account.id) cancelForm();
}

async function scanLatestClipboard(options: { silent: boolean }) {
  const clip = store.clips[0];
  if (!clip) return;
  isScanningClipboard.value = !options.silent;
  try {
    const candidate = await candidateFromClip(clip, "clipboard", t("apps.mfa.sourceClipboard"));
    if (!candidate || ignoredCandidateKeys.value.has(candidateKey(candidate))) return;
    pendingCandidate.value = candidate;
    if (!options.silent) {
      applyCandidate(candidate);
    }
  } catch (unknownError) {
    if (!options.silent) error.value = String(unknownError);
  } finally {
    isScanningClipboard.value = false;
  }
}

async function scanHistory() {
  isScanningHistory.value = true;
  error.value = "";
  historyCandidates.value = [];
  try {
    const page = await ipasteApi.listClips(0, historyScanLimit.value, "");
    const candidates: MfaCandidate[] = [];
    const keys = new Set<string>();
    for (const clip of page.clips) {
      const candidate = await candidateFromClip(
        clip,
        "history",
        t("apps.mfa.sourceHistory", { count: historyScanLimit.value }),
      );
      if (!candidate) continue;
      const key = candidateKey(candidate);
      if (keys.has(key)) continue;
      keys.add(key);
      candidates.push(candidate);
    }
    historyCandidates.value = candidates;
    if (!candidates.length) {
      notice.value = t("apps.mfa.noHistoryQr");
    }
  } catch (unknownError) {
    error.value = String(unknownError);
  } finally {
    isScanningHistory.value = false;
  }
}

function openUploadPicker() {
  uploadInput.value?.click();
}

async function handleUpload(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (!file) return;

  isScanningUpload.value = true;
  error.value = "";
  try {
    const qrText = await decodeQrFromFile(file);
    const candidate = candidateFromText(qrText ?? "", "upload", t("apps.mfa.sourceUpload"));
    if (!candidate) {
      error.value = t("apps.mfa.errorQrNotFound");
      return;
    }
    pendingCandidate.value = candidate;
    applyCandidate(candidate);
  } catch (unknownError) {
    error.value = String(unknownError);
  } finally {
    isScanningUpload.value = false;
  }
}

async function candidateFromClip(clip: ClipItem, source: CandidateSource, sourceLabel: string) {
  if (clip.clipType === "image") {
    const qrText = await decodeQrFromClip(clip).catch(() => null);
    return candidateFromText(qrText ?? "", source, sourceLabel, clip.previewText);
  }

  return candidateFromText(clip.text || clip.previewText, source, sourceLabel, clip.previewText);
}

function candidateFromText(value: string, source: CandidateSource, sourceLabel: string, previewText = ""): MfaCandidate | null {
  const parsed = parseOtpAuthUri(value);
  if (!parsed) return null;
  return {
    id: `${source}-${parsed.secret}-${parsed.name}-${parsed.issuer ?? ""}`,
    input: parsed,
    source,
    sourceLabel,
    previewText,
  };
}

function applyCandidate(candidate: MfaCandidate) {
  Object.assign(draft, {
    name: candidate.input.name,
    issuer: candidate.input.issuer ?? "",
    description: draft.description,
    secret: candidate.input.secret,
    algorithm: candidate.input.algorithm ?? "SHA1",
    digits: candidate.input.digits ?? 6,
    period: candidate.input.period ?? 30,
    sourceUri: candidate.input.sourceUri ?? "",
  });
  formOpen.value = true;
  editingId.value = null;
}

function ignoreCandidate() {
  const candidate = pendingCandidate.value;
  if (candidate) {
    ignoredCandidateKeys.value = new Set([...ignoredCandidateKeys.value, candidateKey(candidate)]);
  }
  pendingCandidate.value = null;
}

function applyUriFromSecret() {
  const parsed = parseOtpAuthUri(draft.secret);
  if (!parsed) return;
  applyCandidate({
    id: `manual-${parsed.secret}`,
    input: parsed,
    source: "clipboard",
    sourceLabel: t("apps.mfa.sourceManual"),
  });
}

function candidateKey(candidate: MfaCandidate) {
  return `${candidate.input.secret}:${candidate.input.name}:${candidate.input.issuer ?? ""}`;
}

function emptyDraft(): MfaDraft {
  return {
    name: "",
    issuer: "",
    description: "",
    secret: "",
    algorithm: "SHA1",
    digits: 6,
    period: 30,
    sourceUri: "",
  };
}
</script>

<template>
  <section class="mfa-manager" :class="{ 'mfa-manager-editing': formOpen }" @click="dismissAccountContextMenu">
    <header class="mfa-toolbar">
      <div class="mfa-toolbar-leading">
        <button
          type="button"
          class="app-center-back-button"
          :aria-label="t('appCenter.back')"
          :data-tooltip="t('appCenter.back')"
          @click.stop="emit('back')"
        >
          <ChevronLeft class="size-4" />
        </button>
        <div class="mfa-toolbar-copy">
          <span class="mfa-toolbar-icon" aria-hidden="true">
            <ShieldCheck class="size-4" />
          </span>
          <div class="min-w-0">
            <h2>{{ t("apps.mfa.managerTitle") }}</h2>
            <p class="mfa-toolbar-kicker">{{ accountCountLabel }}</p>
          </div>
        </div>
      </div>
      <button v-if="!formOpen" type="button" class="mfa-primary-button" @click="startCreate">
        <Plus class="size-4" />
        <span>{{ t("apps.mfa.add") }}</span>
      </button>
    </header>

    <div v-if="pendingCandidate" class="mfa-detection">
      <QrCode class="size-5" />
      <div class="min-w-0 flex-1">
        <strong>{{ t("apps.mfa.detectedTitle") }}</strong>
        <p>{{ pendingCandidate.sourceLabel }} · {{ pendingCandidate.input.issuer || pendingCandidate.input.name }}</p>
      </div>
      <button type="button" class="mfa-small-button" @click="applyCandidate(pendingCandidate)">
        <Check class="size-3.5" />
        <span>{{ t("apps.mfa.useDetected") }}</span>
      </button>
      <button type="button" class="mfa-icon-button" :aria-label="t('common.close')" @click="ignoreCandidate">
        <X class="size-4" />
      </button>
    </div>

    <div v-if="error" class="mfa-message mfa-message-error" role="alert">{{ error }}</div>
    <div v-else-if="notice" class="mfa-message" role="status" aria-live="polite">{{ notice }}</div>

    <div class="mfa-layout" :class="{ 'mfa-layout-editing': formOpen }">
      <section v-if="!formOpen" class="mfa-account-list subtle-scrollbar" :aria-label="t('apps.mfa.listLabel')">
        <div v-if="isLoading" class="mfa-loading-list">
          <div v-for="index in 4" :key="index" class="mfa-loading-row" />
        </div>

        <div v-else-if="accounts.length" class="mfa-account-stack">
          <article
            v-for="account in accounts"
            :key="account.id"
            class="mfa-account-row"
            :class="{ 'mfa-account-row-menu-open': accountContextMenu?.account.id === account.id }"
            @contextmenu.prevent.stop="openAccountContextMenu(account, $event)"
          >
            <div class="mfa-account-identity">
              <span class="mfa-account-avatar" aria-hidden="true">
                <ShieldCheck class="size-4" />
              </span>
              <div class="mfa-account-main">
                <div class="mfa-account-title-line">
                  <strong>{{ account.name }}</strong>
                  <span v-if="account.issuer">{{ account.issuer }}</span>
                </div>
                <p v-if="account.description">{{ account.description }}</p>
              </div>
            </div>

            <div class="mfa-account-controls">
              <button
                type="button"
                class="mfa-code-shell"
                :class="{
                  'mfa-code-shell-error': codeErrorById[account.id],
                  'mfa-code-shell-copied': copiedAccountId === account.id,
                }"
                :style="{ '--mfa-progress': progressPercentFor(account) }"
                :aria-label="t('common.copy')"
                :data-tooltip="t('common.copy')"
                @click.stop="copyAccountCodeFromCard(account)"
              >
                <Check v-if="copiedAccountId === account.id" class="mfa-code-action-icon size-3.5" />
                <ClipboardCopy v-else class="mfa-code-action-icon size-3.5" />
                <span class="mfa-code-value">{{ codeById[account.id] || "------" }}</span>
                <span
                  v-if="!codeErrorById[account.id]"
                  class="mfa-code-timer"
                  :class="`mfa-code-timer-${timerStageFor(account)}`"
                  aria-hidden="true"
                >
                  <span>{{ remainingFor(account) }}</span>
                </span>
                <span v-else class="mfa-code-error">{{ t("apps.mfa.codeError") }}</span>
              </button>

              <button
                type="button"
                class="mfa-paste-button"
                :aria-label="t('apps.mfa.pasteCode')"
                :data-tooltip="t('apps.mfa.pasteCode')"
                @click.stop="pasteAccountCodeFromCard(account)"
              >
                <CornerDownLeft class="size-3.5" />
                <span>{{ t("common.paste") }}</span>
              </button>

              <button
                type="button"
                class="mfa-icon-button mfa-more-button"
                :aria-label="t('apps.mfa.moreActions')"
                :aria-expanded="accountContextMenu?.account.id === account.id"
                aria-haspopup="menu"
                :data-tooltip="t('apps.mfa.moreActions')"
                @click.stop="toggleAccountContextMenu(account, $event)"
              >
                <MoreHorizontal class="size-4" />
              </button>
            </div>
          </article>
        </div>

        <div v-else class="mfa-empty">
          <QrCode class="size-10" />
          <h3>{{ t("apps.mfa.emptyTitle") }}</h3>
          <p>{{ t("apps.mfa.emptyDescription") }}</p>
          <button v-if="!formOpen" type="button" class="mfa-primary-button" @click="startCreate">
            <Plus class="size-4" />
            <span>{{ t("apps.mfa.addFirst") }}</span>
          </button>
        </div>
      </section>

      <aside v-else class="mfa-editor mfa-editor-open">
        <header class="mfa-editor-header">
          <div class="min-w-0">
            <h3>{{ editingId ? t("apps.mfa.editTitle") : t("apps.mfa.createTitle") }}</h3>
            <p>{{ t("apps.mfa.createDescription") }}</p>
          </div>
          <button v-if="formOpen" type="button" class="mfa-icon-button" :aria-label="t('common.close')" @click="cancelForm">
            <X class="size-4" />
          </button>
        </header>

        <div class="mfa-editor-body subtle-scrollbar">
          <form class="mfa-form" @submit.prevent="saveDraft">
            <section class="mfa-form-section">
              <header class="mfa-form-section-header">
                <ShieldCheck class="size-4" />
                <div>
                  <h4>{{ t("apps.mfa.accountDetails") }}</h4>
                  <p>{{ t("apps.mfa.accountDetailsDescription") }}</p>
                </div>
              </header>

              <div class="mfa-field-grid">
                <label class="mfa-field">
                  <span>{{ t("apps.mfa.fieldName") }}</span>
                  <input v-model.trim="draft.name" type="text" maxlength="80" autocomplete="off" :placeholder="t('apps.mfa.placeholderName')" />
                </label>
                <label class="mfa-field">
                  <span>{{ t("apps.mfa.fieldIssuer") }}</span>
                  <input v-model.trim="draft.issuer" type="text" maxlength="80" autocomplete="off" :placeholder="t('apps.mfa.placeholderIssuer')" />
                </label>
              </div>

              <label class="mfa-field">
                <span>{{ t("apps.mfa.fieldDescription") }}</span>
                <input v-model.trim="draft.description" type="text" maxlength="240" autocomplete="off" :placeholder="t('apps.mfa.placeholderDescription')" />
              </label>
            </section>

            <section class="mfa-form-section">
              <header class="mfa-form-section-header">
                <KeyRound class="size-4" />
                <div>
                  <h4>{{ t("apps.mfa.secretAndImport") }}</h4>
                  <p>{{ t("apps.mfa.secretAndImportDescription") }}</p>
                </div>
              </header>

              <label class="mfa-field">
                <span>{{ t("apps.mfa.fieldSecret") }}</span>
                <textarea
                  v-model.trim="draft.secret"
                  rows="3"
                  spellcheck="false"
                  autocomplete="off"
                  :placeholder="t('apps.mfa.placeholderSecret')"
                  @blur="applyUriFromSecret"
                />
              </label>

              <div class="mfa-source-tools">
                <button type="button" class="mfa-small-button" :disabled="isScanningClipboard" @click="scanLatestClipboard({ silent: false })">
                  <RefreshCw class="size-3.5" :class="{ 'mfa-spin': isScanningClipboard }" />
                  <span>{{ t("apps.mfa.scanClipboard") }}</span>
                </button>
                <button type="button" class="mfa-small-button" :disabled="isScanningUpload" @click="openUploadPicker">
                  <Upload class="size-3.5" />
                  <span>{{ t("apps.mfa.uploadQr") }}</span>
                </button>
                <input ref="uploadInput" class="hidden" type="file" accept="image/*" @change="handleUpload" />
              </div>

              <div class="mfa-history-scan">
                <div class="mfa-history-controls">
                  <History class="size-4" />
                  <select v-model.number="historyScanLimit" :aria-label="t('apps.mfa.historyRange')">
                    <option v-for="option in scanLimitOptions" :key="option" :value="option">
                      {{ t("apps.mfa.historyOption", { count: option }) }}
                    </option>
                  </select>
                  <button type="button" class="mfa-small-button" :disabled="isScanningHistory" @click="scanHistory">
                    <RefreshCw class="size-3.5" :class="{ 'mfa-spin': isScanningHistory }" />
                    <span>{{ t("apps.mfa.scanHistory") }}</span>
                  </button>
                </div>

                <div v-if="historyCandidates.length" class="mfa-candidate-list">
                  <button
                    v-for="candidate in historyCandidates"
                    :key="candidate.id"
                    type="button"
                    class="mfa-candidate-row"
                    @click="applyCandidate(candidate)"
                  >
                    <span>
                      <strong>{{ candidate.input.name }}</strong>
                      <small>{{ candidate.input.issuer || candidate.sourceLabel }}</small>
                    </span>
                    <Check class="size-4" />
                  </button>
                </div>
              </div>
            </section>

            <details class="mfa-advanced">
              <summary>
                <span class="mfa-advanced-title">
                  <SlidersHorizontal class="size-4" />
                  <span>{{ t("apps.mfa.advancedSettings") }}</span>
                </span>
                <small>{{ draft.algorithm }} · {{ draft.digits }} · {{ draft.period }}s</small>
              </summary>
              <div class="mfa-advanced-body">
                <p>{{ t("apps.mfa.advancedSettingsDescription") }}</p>
                <div class="mfa-field-grid mfa-field-grid-compact">
                  <label class="mfa-field">
                    <span>{{ t("apps.mfa.fieldAlgorithm") }}</span>
                    <select v-model="draft.algorithm">
                      <option v-for="algorithm in algorithmOptions" :key="algorithm" :value="algorithm">{{ algorithm }}</option>
                    </select>
                  </label>
                  <label class="mfa-field">
                    <span>{{ t("apps.mfa.fieldDigits") }}</span>
                    <input v-model.number="draft.digits" type="number" min="6" max="8" :placeholder="t('apps.mfa.placeholderDigits')" />
                  </label>
                  <label class="mfa-field">
                    <span>{{ t("apps.mfa.fieldPeriod") }}</span>
                    <input v-model.number="draft.period" type="number" min="10" max="120" :placeholder="t('apps.mfa.placeholderPeriod')" />
                  </label>
                </div>
              </div>
            </details>

            <footer class="mfa-form-actions">
              <button type="button" class="mfa-secondary-button" @click="cancelForm">{{ t("common.cancel") }}</button>
              <button type="submit" class="mfa-primary-button" :disabled="saveDisabled">
                <span>{{ isSaving ? t("common.saving") : t("common.save") }}</span>
              </button>
            </footer>
          </form>
        </div>
      </aside>
    </div>

    <div
      v-if="accountContextMenu"
      ref="accountContextMenuElement"
      class="clip-context-menu mfa-context-menu"
      :style="{ left: `${accountContextMenu.x}px`, top: `${accountContextMenu.y}px` }"
      role="menu"
      :aria-label="t('apps.mfa.moreActions')"
      @click.stop
      @keydown="handleAccountMenuKeydown"
      @contextmenu.prevent.stop
      @mouseleave="pendingDeleteId = null"
    >
      <button type="button" class="context-menu-item context-menu-item-strong" role="menuitem" @click="pasteAccountFromContext">
        <CornerDownLeft class="size-4" />
        <span>{{ t("common.paste") }}</span>
      </button>
      <button type="button" class="context-menu-item" role="menuitem" @click="copyAccountFromContext">
        <ClipboardCopy class="size-4" />
        <span>{{ t("common.copy") }}</span>
      </button>
      <div class="context-menu-separator" />
      <button type="button" class="context-menu-item" role="menuitem" @click="editAccountFromContext">
        <Pencil class="size-4" />
        <span>{{ t("common.edit") }}</span>
      </button>
      <div class="context-menu-separator" />
      <button
        type="button"
        class="context-menu-item context-menu-item-danger"
        :class="{ 'context-menu-item-confirm': pendingDeleteId === accountContextMenu.account.id }"
        role="menuitem"
        @click="deleteAccountFromContext"
        @mouseleave="pendingDeleteId = null"
      >
        <Trash2 class="size-4" />
        <span>{{ accountDeleteLabel(accountContextMenu.account) }}</span>
      </button>
    </div>
  </section>
</template>
