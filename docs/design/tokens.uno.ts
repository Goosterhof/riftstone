// The Riftstone — design tokens, direction RIFT-LIT BASALT.
// Source of truth: documents/design-systems/00015-system-riftstone.md §4–§6.
//
// Paste into uno.config.ts:
//   import { riftColors, riftFonts, riftRootCss } from "./docs/design/tokens.uno";
//   theme: { colors: riftColors, fontFamily: riftFonts },
//   preflights: [{ getCSS: () => riftRootCss }, /* the reduced-motion floor stays */],
//
// The two exports below MUST agree value for value. tests can assert it:
// every `rs.<key>` in riftColors appears as `--rs-<key>: <same hex>` in riftRootCss.
// Contrast ratios are [derived:measured] — scratchpad wcag2.py, WCAG 2.x luminance;
// the full matrix is in the design system §4.3.

export const riftColors = {
  rs: {
    // grounds — cold = the Rift (other worlds), warm = your world
    void: "#0A0E13", //     the Rift itself: app ground behind results
    mist: "#111821", //     option cards in the Rift
    misthi: "#18222E", //   card hover / keyboard-selected card
    basalt: "#1F2226", //   the Riftstone pillar
    well: "#121417", //     carved wells in the stone (search field, inputs)
    hearth: "#18130D", //   your world: the detail pane, the Installed page
    hearthhi: "#231C13", // raised row in your world (plan rows, load-order rows)
    // ink
    text: "#ECE6D8", //     bone — body text everywhere (12.83:1 worst, on basalt)
    muted: "#A9B0B8", //    secondary text (7.29:1 worst)
    faint: "#8A929C", //    tertiary / timestamps (5.07:1 worst — still AA body)
    // signal — each one is a STATE, never decoration
    rift: "#8FE3F2", //     not-yet-installed, search matches, focus ring (10.98:1 worst)
    world: "#EBC46F", //    installed — it is in your world (9.63:1 worst)
    ember: "#FF7B66", //    conflict, failure (6.29:1 worst)
    warn: "#FFAE57", //     a switch is off, game patched since install (8.70:1 worst)
    verdant: "#B5E07C", //  update available on Nexus (10.58:1 worst)
    ongold: "#1A1408", //   text on any filled signal chip (7.21:1 worst, on ember)
    riftdeep: "#1E5560", // filled primary button ground (text on it 6.69:1)
    // lines
    edge: "#3A414A", //     decorative hairline only (1.55–1.88:1 — never a control boundary)
    edgehi: "#6E7883", //   control boundaries: inputs, buttons, cards' keyboard frame (3.56:1 worst, WCAG 1.4.11)
  },
} as const;

export const riftFonts = {
  inscription: ["Cinzel", "Georgia", "serif"], //            @fontsource/cinzel 5.3.0 — weights 600, 700
  sans: ["Alegreya Sans", "Segoe UI", "sans-serif"], //       @fontsource/alegreya-sans 5.3.0 — 400, 500, 700
  mono: ["JetBrains Mono", "Consolas", "monospace"], //      @fontsource/jetbrains-mono 5.3.0 — 400, 600
};

export const riftRootCss = `
:root {
  --rs-void: #0A0E13;
  --rs-mist: #111821;
  --rs-misthi: #18222E;
  --rs-basalt: #1F2226;
  --rs-well: #121417;
  --rs-hearth: #18130D;
  --rs-hearthhi: #231C13;
  --rs-text: #ECE6D8;
  --rs-muted: #A9B0B8;
  --rs-faint: #8A929C;
  --rs-rift: #8FE3F2;
  --rs-world: #EBC46F;
  --rs-ember: #FF7B66;
  --rs-warn: #FFAE57;
  --rs-verdant: #B5E07C;
  --rs-ongold: #1A1408;
  --rs-riftdeep: #1E5560;
  --rs-edge: #3A414A;
  --rs-edgehi: #6E7883;

  --rs-font-inscription: "Cinzel", Georgia, serif;
  --rs-font-sans: "Alegreya Sans", "Segoe UI", sans-serif;
  --rs-font-mono: "JetBrains Mono", Consolas, monospace;

  /* type — major third (1.25) from a 14px body */
  --rs-text-xs: 0.75rem;      /* 12px  — mono badges, path fragments */
  --rs-text-sm: 0.875rem;     /* 14px  — body, card text */
  --rs-text-md: 1.09375rem;   /* 17.5px — card titles */
  --rs-text-lg: 1.3671875rem; /* 21.9px — pane titles */
  --rs-text-xl: 1.708984rem;  /* 27.3px — page inscriptions */
  --rs-text-2xl: 2.136230rem; /* 34.2px — empty-state inscriptions */
  --rs-text-display: 2.670288rem; /* 42.7px — the wordmark */

  /* space — 4px base */
  --rs-space-1: 0.25rem;
  --rs-space-2: 0.5rem;
  --rs-space-3: 0.75rem;
  --rs-space-4: 1rem;
  --rs-space-5: 1.25rem;
  --rs-space-6: 1.5rem;
  --rs-space-8: 2rem;
  --rs-space-12: 3rem;

  /* layout — see §8.0 fit sum */
  --rs-pillar-w: 288px;
  --rs-pillar-w-narrow: 248px;
  --rs-world-w: 440px;

  /* radius — stone is chiselled, not rounded */
  --rs-radius-0: 0;
  --rs-radius-chip: 2px;

  /* shadow — one hard shadow, cast by the stone onto the Rift */
  --rs-shadow-stone: 6px 0 0 rgba(0, 0, 0, 0.55);
  --rs-glow-rift: 0 0 0 1px #8FE3F2, 0 0 18px rgba(143, 227, 242, 0.28);

  /* motion */
  --rs-dur-quick: 120ms;
  --rs-dur-settle: 220ms;
  --rs-dur-answer: 640ms;
  --rs-dur-crossing: 420ms;
  --rs-ease-carve: cubic-bezier(0.2, 0.7, 0.1, 1);
  --rs-ease-answer: cubic-bezier(0.16, 1, 0.3, 1);

  /* z */
  --rs-z-pillar: 10;
  --rs-z-world-overlay: 30;
  --rs-z-toast: 50;
}
`;
