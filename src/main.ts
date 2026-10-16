import { createApp } from "vue";
// The reset lands BEFORE the uno layer (Gadget Containment Protocol): without
// it WebView2 paints every <button> in the UA's buttonface gray.
import "@unocss/reset/tailwind.css";
import "virtual:uno.css";
// Bundled faces (CSP font-src 'self' data:), latin subset, only the weights
// Design System #00015 §5 names: Cinzel 600/700 (inscription, stone only),
// Alegreya Sans 400/500/700 (everything you read), JetBrains Mono 400/600
// (paths, badges, carved counts).
import "@fontsource/cinzel/latin-600.css";
import "@fontsource/cinzel/latin-700.css";
import "@fontsource/alegreya-sans/latin-400.css";
import "@fontsource/alegreya-sans/latin-500.css";
import "@fontsource/alegreya-sans/latin-700.css";
import "@fontsource/jetbrains-mono/latin-400.css";
import "@fontsource/jetbrains-mono/latin-600.css";
import "./styles/riftstone.css";
import App from "./App.vue";

createApp(App).mount("#app");
