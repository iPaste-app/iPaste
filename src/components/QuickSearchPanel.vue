<script setup lang="ts">
import { Code, File, Image, Link, Palette, Search, Type, X } from "lucide-vue-next";
import { computed, nextTick, onMounted, onScopeDispose, ref, useTemplateRef, watch } from "vue";
import { useClipSearch } from "../composables/useClipSearch";
import { t } from "../i18n";
import { clipFileName } from "../lib/clipFile";
import { typeLabel } from "../lib/format";
import { useIpasteStore } from "../stores/ipasteStore";
import type { ClipViewItem } from "../types";

const props = defineProps<{ appCenter: boolean }>();
const appFilter = defineModel<string>("appFilter", { required: true });
const emit = defineEmits<{ dismiss: []; reveal: [] }>();

const store = useIpasteStore();
const { query, results, isSearching, searchFailed, loadMore } = useClipSearch();
const panel = useTemplateRef<HTMLElement>("panel");
const input = useTemplateRef<HTMLInputElement>("input");
const resultList = useTemplateRef<HTMLElement>("resultList");
const activeIndex = ref(0);
const isRevealing = ref(false);
const revealFailed = ref(false);
const isComposing = ref(false);
let revealRequest: AbortController | null = null;

const inputValue = computed({
  get: () => props.appCenter ? appFilter.value : query.value,
  set: (value) => {
    if (props.appCenter) appFilter.value = value;
    else query.value = value;
  },
});
const placeholder = computed(() => t(props.appCenter ? "shortcut.search" : "topBar.searchPlaceholder"));
const selectedIndex = computed(() => Math.min(activeIndex.value, results.value.length - 1));
const status = computed(() => {
  if (isRevealing.value) return t("quickSearch.loading");
  if (revealFailed.value) return t("quickSearch.revealError");
  if (searchFailed.value) return t("quickSearch.loadError");
  if (isSearching.value || store.isLoading) return t("quickSearch.loading");
  if (!results.value.length) return t(query.value.trim() ? "quickSearch.noResults" : "empty.title");
  return "";
});
const icons = { text: Type, link: Link, color: Palette, image: Image, file: File, html: Code };

watch([query, () => store.selectedCategoryId], () => {
  activeIndex.value = 0;
  revealFailed.value = false;
  if (resultList.value) resultList.value.scrollTop = 0;
});

onMounted(() => focusInput());
onScopeDispose(() => revealRequest?.abort());

function focusInput(select = false) {
  input.value?.focus({ preventScroll: true });
  if (select) input.value?.select();
}

function dismiss() {
  revealRequest?.abort();
  emit("dismiss");
}

function optionId(index: number) {
  return `quick-search-result-${index}`;
}

function resultLabel(item: ClipViewItem) {
  return item.displayName?.trim() || (item.clipType === "file" ? clipFileName(item.text) : item.previewText.trim())
    || typeLabel(item.clipType);
}

function loadNearEnd() {
  const list = resultList.value;
  if (list && list.scrollHeight - list.scrollTop - list.clientHeight < 80) loadMore();
}

function moveSelection(delta: number) {
  if (isRevealing.value || !results.value.length) return;
  activeIndex.value = Math.max(0, Math.min(selectedIndex.value + delta, results.value.length - 1));
  if (delta > 0 && activeIndex.value >= results.value.length - 3) loadMore();
  void nextTick(() => {
    resultList.value?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: "nearest" });
  });
}

async function choose(index: number) {
  const item = results.value[index];
  if (!item || isRevealing.value || isComposing.value) return;

  activeIndex.value = index;
  revealFailed.value = false;
  isRevealing.value = true;
  const request = new AbortController();
  revealRequest = request;
  try {
    const found = await store.revealClip(item, request.signal);
    if (request.signal.aborted) return;
    if (found) emit("reveal");
    else revealFailed.value = true;
  } finally {
    revealRequest = null;
    isRevealing.value = false;
  }
}

function handleKey(key: string, repeat = false) {
  if (isComposing.value) return false;
  if (key === "Escape") {
    if (!repeat) dismiss();
    return true;
  }
  if (props.appCenter) return false;
  if (key === "ArrowDown" || key === "ArrowUp") {
    moveSelection(key === "ArrowDown" ? 1 : -1);
    return true;
  }
  if (key === "Enter") {
    if (!repeat) void choose(selectedIndex.value);
    return true;
  }
  return false;
}

defineExpose({
  focusInput,
  handleKey,
  isComposing: () => isComposing.value,
  containsFocus: () => !!panel.value?.contains(document.activeElement),
});
</script>

<template>
  <div :class="appCenter ? 'app-center-search' : 'quick-search-backdrop'" @click.self.stop="dismiss">
    <section
      id="quick-search-panel"
      ref="panel"
      class="quick-search-panel"
      :class="{ 'quick-search-panel-filter': appCenter }"
      role="dialog"
      :aria-label="t('shortcut.search')"
    >
      <div class="quick-search-field">
        <Search class="size-4 shrink-0 text-slate-500" aria-hidden="true" />
        <input
          ref="input"
          v-model="inputValue"
          type="text"
          class="min-w-0 flex-1 bg-transparent text-sm text-slate-950 placeholder:text-slate-500"
          :placeholder="placeholder"
          :aria-label="placeholder"
          :role="appCenter ? undefined : 'combobox'"
          :aria-autocomplete="appCenter ? undefined : 'list'"
          :aria-expanded="appCenter ? undefined : true"
          :aria-controls="appCenter ? undefined : 'quick-search-results'"
          :aria-activedescendant="!appCenter && selectedIndex >= 0 ? optionId(selectedIndex) : undefined"
          :readonly="isRevealing"
          spellcheck="false"
          autocomplete="off"
          @compositionstart="isComposing = true"
          @compositionend="isComposing = false"
          @change="isComposing = false"
        />
        <button type="button" class="icon-button quick-search-close" :aria-label="t('common.close')" @click="dismiss">
          <X class="size-3.5" aria-hidden="true" />
        </button>
      </div>

      <template v-if="!appCenter">
        <div ref="resultList" class="quick-search-results" @scroll="loadNearEnd">
          <div id="quick-search-results" role="listbox" :aria-label="t('shortcut.search')" :aria-busy="isSearching">
            <div
              v-for="(item, index) in results"
              :id="optionId(index)"
              :key="item.id"
              class="quick-search-result"
              role="option"
              :aria-selected="index === selectedIndex"
              @mousedown.prevent
              @click="choose(index)"
            >
              <span class="quick-search-result-icon" :class="`quick-search-result-${item.clipType}`">
                <component :is="icons[item.clipType]" class="size-3.5" :aria-label="typeLabel(item.clipType)" />
              </span>
              <span class="min-w-0 truncate">{{ resultLabel(item) }}</span>
            </div>
          </div>
        </div>

        <p v-if="status" class="quick-search-status" role="status">{{ status }}</p>
        <footer v-else-if="results.length" class="quick-search-footer">
          <span><kbd>↑</kbd> <kbd>↓</kbd> {{ t('quickSearch.move') }}</span>
          <span><kbd>Enter</kbd> {{ t('quickSearch.reveal') }}</span>
        </footer>
      </template>
    </section>
  </div>
</template>

<style scoped>
.quick-search-backdrop {
  position: absolute;
  inset: 0;
  z-index: 50;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  background: rgba(30, 41, 59, 0.15);
  padding: 1rem 0.625rem;
}

.quick-search-panel {
  display: flex;
  width: min(100%, 23.25rem);
  max-height: 100%;
  min-height: 0;
  flex-direction: column;
  gap: 0.375rem;
  border: 1px solid #dbe3ee;
  border-radius: 0.875rem;
  background: #fff;
  padding: 0.625rem;
  box-shadow: 0 12px 32px rgba(15, 23, 42, 0.16);
}

.app-center-search {
  flex: none;
  border-bottom: 1px solid #e2e8f0;
  background: var(--theme-topbar-background);
  padding: 0.625rem 1rem;
}

.quick-search-panel-filter {
  border: 0;
  background: transparent;
  padding: 0;
  box-shadow: none;
}

.quick-search-field {
  display: flex;
  height: 2.375rem;
  flex: none;
  align-items: center;
  gap: 0.5rem;
  border: 1px solid #dbe3ee;
  border-radius: 0.5rem;
  background: #f8fafc;
  padding: 0 0.375rem 0 0.625rem;
}

.quick-search-field:focus-within {
  border-color: var(--theme-accent);
  background: #fff;
  box-shadow: 0 0 0 2px var(--theme-accent-selected);
}

.quick-search-close {
  width: 1.625rem;
  height: 1.625rem;
  flex: none;
  border-radius: 0.375rem;
}

.quick-search-results {
  max-height: 22rem;
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  scrollbar-width: thin;
}

.quick-search-result {
  display: flex;
  min-height: 2.1875rem;
  align-items: center;
  gap: 0.5rem;
  border-radius: 0.375rem;
  padding: 0.25rem 0.5rem;
  color: #334155;
  cursor: pointer;
  font-size: 0.8125rem;
}

.quick-search-result:hover {
  background: #f1f5f9;
}

.quick-search-result[aria-selected="true"] {
  background: var(--theme-accent-selected);
  color: var(--theme-accent-text);
}

.quick-search-result-icon {
  display: grid;
  width: 1.5rem;
  height: 1.5rem;
  flex: none;
  place-items: center;
  border-radius: 0.375rem;
  background: #f1f5f9;
  color: #64748b;
}

.quick-search-result-link { background: #eff6ff; color: #2563eb; }
.quick-search-result-image { background: #f5f3ff; color: #7c3aed; }
.quick-search-result-color { background: #fdf2f8; color: #be185d; }

.quick-search-status {
  flex: none;
  padding: 0.375rem 0.5rem;
  color: #64748b;
  font-size: 0.8125rem;
}

.quick-search-footer {
  display: flex;
  flex: none;
  justify-content: space-between;
  gap: 0.5rem;
  border-top: 1px solid #f1f5f9;
  padding: 0.375rem 0.5rem 0;
  color: #64748b;
  font-size: 0.6875rem;
}

.quick-search-footer kbd {
  border: 1px solid #e2e8f0;
  border-radius: 0.25rem;
  background: #f8fafc;
  padding: 0 0.25rem;
}
</style>
