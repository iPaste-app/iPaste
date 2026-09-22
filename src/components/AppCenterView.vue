<script setup lang="ts">
import { computed, ref } from "vue";
import { ShieldCheck } from "lucide-vue-next";
import { builtinApps, type AppDefinition, type BuiltinAppId } from "../lib/appCenter";
import { t } from "../i18n";
import MfaManagerApp from "./MfaManagerApp.vue";

const props = withDefaults(defineProps<{ search?: string }>(), { search: "" });
const emit = defineEmits<{ clearSearch: [] }>();
const activeAppId = ref<BuiltinAppId | null>(null);
const mfaManager = ref<InstanceType<typeof MfaManagerApp> | null>(null);
const activeApp = computed(() => builtinApps.find((app) => app.id === activeAppId.value) ?? null);
const visibleApps = computed(() => builtinApps.filter((app) =>
  `${t(app.nameKey)} ${t(app.descriptionKey)}`.toLocaleLowerCase().includes(props.search.trim().toLocaleLowerCase()),
));

function openApp(app: AppDefinition) {
  emit("clearSearch");
  activeAppId.value = app.id;
}

function closeApp() {
  emit("clearSearch");
  activeAppId.value = null;
}

defineExpose({
  handleNativePanelKey: (key: string) => mfaManager.value?.handleNativePanelKey(key) ?? false,
});
</script>

<template>
  <section class="app-center-view">
    <template v-if="!activeApp">
      <header class="app-center-header">
        <div class="app-center-title-copy">
          <h1>{{ t("appCenter.title") }}</h1>
          <p>{{ t("appCenter.description") }}</p>
        </div>
      </header>

      <div class="app-center-grid">
        <button
          v-for="app in visibleApps"
          :key="app.id"
          type="button"
          class="app-card"
          :style="{ '--app-accent': app.accent }"
          @click="openApp(app)"
        >
          <span class="app-card-topline">
            <span class="app-card-icon">
              <ShieldCheck v-if="app.id === 'mfa-manager'" class="size-5" />
            </span>
            <span class="app-card-title">{{ t(app.nameKey) }}</span>
          </span>
          <span class="app-card-body">
            <span class="app-card-description">{{ t(app.descriptionKey) }}</span>
            <span class="app-card-capabilities">
              <span v-for="capability in app.capabilities" :key="capability">{{ t(capability) }}</span>
            </span>
          </span>
        </button>
      </div>
      <div v-if="!visibleApps.length" class="mfa-empty" role="status">{{ t('apps.mfa.noMatches') }}</div>
    </template>

    <template v-else>
      <MfaManagerApp v-if="activeApp.id === 'mfa-manager'" ref="mfaManager" :search="search" @back="closeApp" />
    </template>
  </section>
</template>
