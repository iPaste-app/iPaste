<script setup lang="ts">
import { computed } from "vue";
import { renderMarkdown } from "../lib/markdown";
import { openExternalUrl } from "../lib/starPrompt";

const props = defineProps<{ source: string }>();
const html = computed(() => renderMarkdown(props.source));

async function openLink(event: MouseEvent) {
  if (event.button !== 0 && event.button !== 1) return;
  const link = event.target instanceof Element ? event.target.closest("a") : null;
  if (!link) return;

  event.preventDefault();
  const href = link.getAttribute("href");
  if (!href || !/^https?:\/\//i.test(href)) return;

  try {
    await openExternalUrl(href);
  } catch (error) {
    console.warn("[ipaste] failed to open Markdown link", error);
  }
}
</script>

<template>
  <div class="markdown-preview" @click="openLink" @auxclick="openLink" v-html="html" />
</template>

<style scoped>
.markdown-preview {
  overflow-wrap: anywhere;
}

.markdown-preview :deep(p),
.markdown-preview :deep(ul),
.markdown-preview :deep(ol),
.markdown-preview :deep(blockquote),
.markdown-preview :deep(pre),
.markdown-preview :deep(table) {
  margin: 0.625rem 0;
  color: inherit;
  font-size: inherit;
  line-height: inherit;
}

.markdown-preview :deep(h1),
.markdown-preview :deep(h2),
.markdown-preview :deep(h3),
.markdown-preview :deep(h4),
.markdown-preview :deep(h5),
.markdown-preview :deep(h6) {
  margin: 0.875rem 0 0.375rem;
  font-size: 0.875rem;
  font-weight: 700;
  line-height: 1.4;
}

.markdown-preview :deep(h1) {
  font-size: 1rem;
}

.markdown-preview :deep(h2) {
  font-size: 0.9375rem;
}

.markdown-preview :deep(ul),
.markdown-preview :deep(ol) {
  padding-left: 1.375rem;
}

.markdown-preview :deep(ul) {
  list-style: disc;
}

.markdown-preview :deep(ol) {
  list-style: decimal;
}

.markdown-preview :deep(li + li) {
  margin-top: 0.25rem;
}

.markdown-preview :deep(a) {
  color: #0f766e;
  text-decoration: underline;
  text-underline-offset: 0.1875rem;
}

.markdown-preview :deep(a:focus-visible) {
  outline: 2px solid #0f766e;
  outline-offset: 2px;
}

.markdown-preview :deep(blockquote) {
  border-left: 3px solid #cbd5e1;
  padding-left: 0.75rem;
  color: #64748b;
}

.markdown-preview :deep(code) {
  border-radius: 0.25rem;
  background: #e2e8f0;
  padding: 0.125rem 0.25rem;
  font-size: 0.9em;
}

.markdown-preview :deep(pre) {
  overflow-x: auto;
  border-radius: 0.375rem;
  background: #e2e8f0;
  padding: 0.625rem;
  white-space: pre;
  overflow-wrap: normal;
}

.markdown-preview :deep(pre code) {
  background: transparent;
  padding: 0;
}

.markdown-preview :deep(table) {
  display: block;
  overflow-x: auto;
  border-collapse: collapse;
}

.markdown-preview :deep(th),
.markdown-preview :deep(td) {
  border: 1px solid #cbd5e1;
  padding: 0.375rem 0.5rem;
}

.markdown-preview :deep(img) {
  max-width: 100%;
  height: auto;
}

.markdown-preview :deep(hr) {
  margin: 0.75rem 0;
  border: 0;
  border-top: 1px solid #cbd5e1;
}

.markdown-preview :deep(> :first-child) {
  margin-top: 0;
}

.markdown-preview :deep(> :last-child) {
  margin-bottom: 0;
}
</style>
