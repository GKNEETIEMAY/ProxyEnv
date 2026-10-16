import { createApp } from "vue";
import App from "./App.vue";
import "./shared/styles/index.css";
import "./shared/theme/palettes.css";
import { applyTheme, readAppearance, readPalette } from "./shared/theme/palette";

applyTheme(document.documentElement, readAppearance(), readPalette(), window.matchMedia("(prefers-color-scheme: dark)").matches);
createApp(App).mount("#app");
