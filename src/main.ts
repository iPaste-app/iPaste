import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import { installTooltipOverflowGuard } from "./lib/tooltips";
import "./styles/main.css";

installTooltipOverflowGuard();

createApp(App).use(createPinia()).mount("#app");
