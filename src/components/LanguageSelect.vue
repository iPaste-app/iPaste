<script setup lang="ts">
import { Check, ChevronDown } from "lucide-vue-next";
import { computed, nextTick, onMounted, onUnmounted, ref, useId } from "vue";
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
const optionRefs = new Map<Language, HTMLButtonElement>();
const isOpen = ref(false);
const activeIndex = ref(0);
const listboxId = useId();
const labelId = `${listboxId}-label`;
const valueId = `${listboxId}-value`;

const selectLabel = computed(() => props.label ?? t("settings.language.title"));
const selectedIndex = computed(() => props.options.findIndex((option) => option.value === props.modelValue));
const selectedOption = computed(() => props.options[selectedIndex.value] ?? props.options[0]);

onMounted(() => {
  document.addEventListener("pointerdown", handleDocumentPointerDown);
});

onUnmounted(() => {
  document.removeEventListener("pointerdown", handleDocumentPointerDown);
});

function setOptionRef(element: unknown, language: Language) {
  if (element instanceof HTMLButtonElement) optionRefs.set(language, element);
  else optionRefs.delete(language);
}

function focusOption(index: number) {
  activeIndex.value = index;
  void nextTick(() => {
    const option = props.options[activeIndex.value];
    if (isOpen.value && option) optionRefs.get(option.value)?.focus();
  });
}

function openMenu() {
  if (!props.options.length) return;
  isOpen.value = true;
  focusOption(Math.max(selectedIndex.value, 0));
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
  focusOption((activeIndex.value + step + props.options.length) % props.options.length);
}

function handleTriggerKeydown(event: KeyboardEvent) {
  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault();
    openMenu();
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
    focusOption(0);
    return;
  }

  if (event.key === "End") {
    event.preventDefault();
    focusOption(props.options.length - 1);
    return;
  }

  if (event.key === "Escape") {
    event.preventDefault();
    event.stopPropagation();
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
      :aria-labelledby="`${labelId} ${valueId}`"
      aria-haspopup="listbox"
      :aria-expanded="isOpen"
      :aria-controls="isOpen ? listboxId : undefined"
      @click="toggleMenu"
      @keydown="handleTriggerKeydown"
    >
      <span :id="labelId" class="sr-only">{{ selectLabel }}</span>
      <span :id="valueId" class="language-select-value" :lang="selectedOption?.value">{{ selectedOption?.label }}</span>
      <ChevronDown class="language-select-chevron size-4" aria-hidden="true" />
    </button>

    <Transition name="language-select-popover">
      <div v-if="isOpen" :id="listboxId" class="language-select-menu subtle-scrollbar" role="listbox" :aria-label="selectLabel">
        <button
          v-for="(option, index) in options"
          :id="`${listboxId}-option-${index}`"
          :key="option.value"
          :ref="(element) => setOptionRef(element, option.value)"
          type="button"
          class="language-select-option"
          :class="{
            'language-select-option-active': activeIndex === index,
            'language-select-option-selected': modelValue === option.value,
          }"
          role="option"
          :tabindex="activeIndex === index ? 0 : -1"
          :aria-selected="modelValue === option.value"
          @click="chooseLanguage(option.value)"
          @keydown="handleOptionKeydown"
          @focus="activeIndex = index"
        >
          <span :lang="option.value">{{ option.label }}</span>
          <Check v-if="modelValue === option.value" class="size-4" aria-hidden="true" />
        </button>
      </div>
    </Transition>
  </div>
</template>
