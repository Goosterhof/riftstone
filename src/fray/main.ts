import { createApp } from "vue";
// The reset lands BEFORE the uno layer (Gadget Containment Protocol), as in
// the main window; the uno layer carries the tokens and the reduced-motion
// floor for this window too.
import "@unocss/reset/tailwind.css";
import "virtual:uno.css";
// The faces THE FRAY speaks in (Design System #00015 §5): Cinzel 600 for its
// inscriptions, Alegreya Sans 400/500/700 for words, JetBrains Mono 400/600
// for every figure.
import "@fontsource/cinzel/latin-600.css";
import "@fontsource/alegreya-sans/latin-400.css";
import "@fontsource/alegreya-sans/latin-500.css";
import "@fontsource/alegreya-sans/latin-700.css";
import "@fontsource/jetbrains-mono/latin-400.css";
import "@fontsource/jetbrains-mono/latin-600.css";
import "./fray.css";
import FrayApp from "./FrayApp.vue";

createApp(FrayApp).mount("#fray-app");
