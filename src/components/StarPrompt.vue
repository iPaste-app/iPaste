<script setup lang="ts">
import { Star } from "lucide-vue-next";
import { t } from "../i18n";

defineProps<{
  open: boolean;
  busy?: boolean;
}>();

const emit = defineEmits<{
  alreadyStarred: [];
  openRepository: [];
  snooze: [];
}>();
</script>

<template>
  <Transition name="star-prompt">
    <aside
      v-if="open"
      class="star-prompt"
      role="region"
      aria-live="polite"
      aria-labelledby="star-prompt-title"
      aria-describedby="star-prompt-description"
      @click.stop
    >
      <div class="star-prompt-content">
        <h2 id="star-prompt-title">{{ t("starPrompt.title") }}</h2>
        <p id="star-prompt-description">{{ t("starPrompt.description") }}</p>
        <div class="star-prompt-actions">
          <button type="button" class="star-prompt-primary" :disabled="busy" @click="emit('openRepository')">
            <Star class="size-4" />
            <span>{{ t("starPrompt.openRepository") }}</span>
          </button>
          <button type="button" class="star-prompt-success" :disabled="busy" @click="emit('alreadyStarred')">
            {{ t("starPrompt.alreadyStarred") }}
          </button>
          <button type="button" class="star-prompt-dismiss" :disabled="busy" @click="emit('snooze')">
            {{ t("starPrompt.remindLater") }}
          </button>
        </div>
      </div>
    </aside>
  </Transition>
</template>
