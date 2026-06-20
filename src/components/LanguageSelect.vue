<script setup lang="ts">
import { Check, ChevronDown } from "lucide-vue-next";
import { computed, nextTick, onBeforeUpdate, onMounted, onUnmounted, ref, watch } from "vue";
import { t } from "../i18n";
import type { Language } from "../types";

type LanguageSelectOption = {
  value: Language;
  label: string;
};

const props = defineProps<{
  modelValue: Language;
  options: LanguageSelectOption[];
  label?: string;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: Language];
}>();

const root = ref<HTMLElement | null>(null);
const trigger = ref<HTMLButtonElement | null>(null);
const optionRefs = ref<HTMLButtonElement[]>([]);
const isOpen = ref(false);
const activeIndex = ref(0);
const listboxId = `language-select-${Math.random().toString(36).slice(2)}`;

const selectLabel = computed(() => props.label ?? t("settings.language.title"));
const selectedIndex = computed(() => props.options.findIndex((option) => option.value === props.modelValue));
const selectedOption = computed(() => props.options[selectedIndex.value] ?? props.options[0]);
const selectedLabel = computed(() => selectedOption.value?.label ?? "");

watch(
  () => props.modelValue,
  () => {
    const index = selectedIndex.value;
    if (index >= 0) {
      activeIndex.value = index;
    }
  },
);

onBeforeUpdate(() => {
  optionRefs.value = [];
});

onMounted(() => {
  document.addEventListener("pointerdown", handleDocumentPointerDown);
});

onUnmounted(() => {
  document.removeEventListener("pointerdown", handleDocumentPointerDown);
});

function setOptionRef(element: unknown, index: number) {
  if (element instanceof HTMLButtonElement) {
    optionRefs.value[index] = element;
  }
}

function normalizedIndex(index: number) {
  if (!props.options.length) return 0;
  return Math.max(0, Math.min(index, props.options.length - 1));
}

function openMenu(index = selectedIndex.value) {
  if (!props.options.length) return;
  activeIndex.value = normalizedIndex(index >= 0 ? index : 0);
  isOpen.value = true;
  void nextTick(() => optionRefs.value[activeIndex.value]?.focus());
}

function closeMenu(focusTrigger = false) {
  isOpen.value = false;
  if (focusTrigger) {
    void nextTick(() => trigger.value?.focus());
  }
}

function toggleMenu() {
  if (isOpen.value) {
    closeMenu();
  } else {
    openMenu();
  }
}

function chooseLanguage(value: Language) {
  if (value !== props.modelValue) {
    emit("update:modelValue", value);
  }
  closeMenu(true);
}

function moveActive(step: number) {
  if (!props.options.length) return;
  activeIndex.value = (activeIndex.value + step + props.options.length) % props.options.length;
  void nextTick(() => optionRefs.value[activeIndex.value]?.focus());
}

function handleTriggerKeydown(event: KeyboardEvent) {
  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault();
    openMenu(selectedIndex.value);
    return;
  }

  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    toggleMenu();
  }
}

function handleOptionKeydown(event: KeyboardEvent) {
  if (event.key === "ArrowDown") {
    event.preventDefault();
    moveActive(1);
    return;
  }

  if (event.key === "ArrowUp") {
    event.preventDefault();
    moveActive(-1);
    return;
  }

  if (event.key === "Home") {
    event.preventDefault();
    activeIndex.value = 0;
    void nextTick(() => optionRefs.value[activeIndex.value]?.focus());
    return;
  }

  if (event.key === "End") {
    event.preventDefault();
    activeIndex.value = props.options.length - 1;
    void nextTick(() => optionRefs.value[activeIndex.value]?.focus());
    return;
  }

  if (event.key === "Escape") {
    event.preventDefault();
    closeMenu(true);
  }
}

function handleFocusOut(event: FocusEvent) {
  const nextTarget = event.relatedTarget;
  if (nextTarget instanceof Node && root.value?.contains(nextTarget)) return;
  closeMenu();
}

function handleDocumentPointerDown(event: PointerEvent) {
  const target = event.target;
  if (target instanceof Node && root.value?.contains(target)) return;
  closeMenu();
}
</script>

<template>
  <div ref="root" class="language-select" :class="{ 'language-select-open': isOpen }" @focusout="handleFocusOut">
    <button
      ref="trigger"
      type="button"
      class="language-select-trigger"
      :aria-label="selectLabel"
      aria-haspopup="listbox"
      :aria-expanded="isOpen"
      :aria-controls="listboxId"
      @click="toggleMenu"
      @keydown="handleTriggerKeydown"
    >
      <span class="language-select-value">{{ selectedLabel }}</span>
      <ChevronDown class="language-select-chevron size-4" aria-hidden="true" />
    </button>

    <Transition name="language-select-popover">
      <div v-if="isOpen" :id="listboxId" class="language-select-menu subtle-scrollbar" role="listbox" :aria-label="selectLabel">
        <button
          v-for="(option, index) in options"
          :id="`${listboxId}-option-${index}`"
          :key="option.value"
          :ref="(element) => setOptionRef(element, index)"
          type="button"
          class="language-select-option"
          :class="{
            'language-select-option-active': activeIndex === index,
            'language-select-option-selected': modelValue === option.value,
          }"
          role="option"
          :aria-selected="modelValue === option.value"
          @click="chooseLanguage(option.value)"
          @keydown="handleOptionKeydown"
          @mouseenter="activeIndex = index"
        >
          <span>{{ option.label }}</span>
          <Check v-if="modelValue === option.value" class="size-4" aria-hidden="true" />
        </button>
      </div>
    </Transition>
  </div>
</template>
