<script setup lang="ts">
// §8.6 — PREFLIGHT, warm: your game. A checklist: one row per check, each a
// status glyph + word + the measured value + its recovery action. Then the
// library roots, the Nexus key and the two settings the user owns.
import { computed } from "vue";
import type { Check, VeilMode } from "@/types/rift";
import { useStone } from "@/composables/useStone";
import { useSettings } from "@/composables/useSettings";
import { useNavigation } from "@/composables/useNavigation";
import { usePreflight, REFRAMEWORK_URL } from "@/composables/usePreflight";
import RiftButton from "@/components/RiftButton.vue";
import RiftInscription from "@/components/RiftInscription.vue";

const stone = useStone();
const nav = useNavigation();
const preflight = usePreflight();
const { settings, setVeil, setAutoCheck } = useSettings();

const o = computed(() => stone.overview.value);
const game = computed(() => o.value?.game ?? null);
const ref_ = computed(() => o.value?.reframework ?? null);
const fluffy = computed(() => o.value?.fluffy ?? null);

type Level = "ok" | "warn" | "error" | "info";
const GLYPH: Record<Level, { glyph: string; word: string }> = {
  ok: { glyph: "✓", word: "OK" },
  warn: { glyph: "⚠", word: "CHECK" },
  error: { glyph: "✗", word: "MISSING" },
  info: { glyph: "·", word: "NOTE" },
};

const OWN_CHECKS = new Set(["game", "build", "reframework", "pakdir", "loose", "fluffy"]);
const spineChecks = computed<Check[]>(() => (o.value?.checks ?? []).filter((c) => !OWN_CHECKS.has(c.id)));

const switchLevel = (on: boolean | null | undefined): Level => (on ? "ok" : "warn");
function switchWord(on: boolean | null | undefined): string {
  if (on === null || on === undefined) return "unknown";
  return on ? "ON" : "OFF";
}

const VEILS: { id: VeilMode; label: string; line: string }[] = [
  { id: "off", label: "Off", line: "Every screenshot shows as it is." },
  {
    id: "adult",
    label: "Adult & unlabeled",
    line: "Blurs the screenshots of mods Nexus flags as adult, and of packages Nexus has not labelled yet, until you hover or focus their card. A veil protects, so the unknown stays veiled: link a package and check for updates, and its veil lifts once Nexus labels it without the adult flag.",
  },
  { id: "all", label: "All", line: "Blurs every screenshot until you hover or focus its card." },
];
const veilLine = computed(
  () => `${VEILS.find((v) => v.id === settings.value.veil)?.line ?? ""} The lightbox always opens unveiled.`,
);

const buildLevel = computed<Level>(() => (game.value?.buildChanged ? "warn" : "ok"));
</script>

<template>
  <main class="pre" aria-label="Preflight and settings">
    <header class="pre__head">
      <h1 class="pre__title rs-inscription">PREFLIGHT</h1>
      <p class="pre__sub">Everything Dragon's Dogma 2 needs before a mod can load, read from your game folder.</p>
    </header>

    <RiftInscription
      v-if="ref_ && !ref_.installed"
      size="md"
      tone="warn"
      inscription="THE BRIDGE IS MISSING"
      body="REFramework isn't in your game folder, so script and loose-file mods won't load. PAK mods won't either: the Alternate PAK Folder lives in REFramework."
    >
      <RiftButton variant="primary" @click="preflight.openUrl(REFRAMEWORK_URL)">Open REFramework on Nexus</RiftButton>
      <RiftButton @click="stone.refreshOverview()">I installed it, check again</RiftButton>
    </RiftInscription>

    <ol class="pre__list">
      <li class="pre__row" :class="`is-${game ? 'ok' : 'error'}`">
        <span class="pre__glyph">{{ GLYPH[game ? "ok" : "error"].glyph }}</span>
        <span class="pre__word rs-mono">{{ GLYPH[game ? "ok" : "error"].word }}</span>
        <span class="pre__what">Game install</span>
        <span class="pre__val rs-mono">{{ game?.root ?? "not found" }}</span>
        <RiftButton size="sm" :busy="preflight.busy.value === 'game'" @click="preflight.pickGameFolder()"
          >Change…</RiftButton
        >
      </li>
      <li v-if="game" class="pre__row" :class="`is-${buildLevel}`">
        <span class="pre__glyph">{{ GLYPH[buildLevel].glyph }}</span>
        <span class="pre__word rs-mono">{{ GLYPH[buildLevel].word }}</span>
        <span class="pre__what">Steam build</span>
        <span class="pre__val rs-mono"
          >{{ game.buildId ?? "unknown"
          }}<template v-if="game.buildChanged">
            · was {{ game.buildChanged.recorded }} at your last install</template
          ></span
        >
        <RiftButton v-if="stone.buildChange.value" size="sm" @click="stone.dismissBuildChange()"
          >I've tested it</RiftButton
        >
      </li>
      <li v-if="ref_" class="pre__row" :class="`is-${ref_.installed ? 'ok' : 'error'}`">
        <span class="pre__glyph">{{ GLYPH[ref_.installed ? "ok" : "error"].glyph }}</span>
        <span class="pre__word rs-mono">{{ GLYPH[ref_.installed ? "ok" : "error"].word }}</span>
        <span class="pre__what">REFramework</span>
        <span class="pre__val rs-mono">{{
          ref_.installed
            ? `${ref_.version ?? "version unknown"} · built ${ref_.buildDate ?? "?"}`
            : "dinput8.dll not in the game folder"
        }}</span>
        <RiftButton v-if="!ref_.installed" size="sm" @click="preflight.openUrl(REFRAMEWORK_URL)">Get it</RiftButton>
      </li>
      <li v-if="ref_?.installed" class="pre__row" :class="`is-${switchLevel(ref_.pakDirectory)}`">
        <span class="pre__glyph">{{ GLYPH[switchLevel(ref_.pakDirectory)].glyph }}</span>
        <span class="pre__word rs-mono">{{ switchWord(ref_.pakDirectory) }}</span>
        <span class="pre__what">Alternate PAK Folder</span>
        <span class="pre__val rs-mono">{{
          ref_.pakDirectory ? "loads pak_mods/" : "Your PAK mods won't load: REFramework's Alternate PAK Folder is off."
        }}</span>
        <RiftButton
          v-if="!ref_.pakDirectory"
          size="sm"
          variant="primary"
          :busy="preflight.busy.value === 'pakDirectory'"
          title="Backs up re2_fw_config.txt first"
          @click="preflight.turnOn('pakDirectory')"
          >Turn it on</RiftButton
        >
      </li>
      <li v-if="ref_?.installed" class="pre__row" :class="`is-${switchLevel(ref_.looseFileLoader)}`">
        <span class="pre__glyph">{{ GLYPH[switchLevel(ref_.looseFileLoader)].glyph }}</span>
        <span class="pre__word rs-mono">{{ switchWord(ref_.looseFileLoader) }}</span>
        <span class="pre__what">Loose File Loader</span>
        <span class="pre__val rs-mono">{{
          ref_.looseFileLoader
            ? "loads natives/"
            : "Loose-file mods won't load: REFramework's Loose File Loader is off."
        }}</span>
        <RiftButton
          v-if="!ref_.looseFileLoader"
          size="sm"
          variant="primary"
          :busy="preflight.busy.value === 'looseFileLoader'"
          title="Backs up re2_fw_config.txt first"
          @click="preflight.turnOn('looseFileLoader')"
          >Turn it on</RiftButton
        >
      </li>
      <li v-if="ref_?.faultyFiles.length" class="pre__row is-warn">
        <span class="pre__glyph">⚠</span>
        <span class="pre__word rs-mono">CHECK</span>
        <span class="pre__what">Faulty files</span>
        <span class="pre__val rs-mono">REFramework flagged {{ ref_.faultyFiles.join(", ") }}</span>
      </li>
      <li class="pre__row" :class="`is-${fluffy ? (fluffy.unadoptedCount ? 'info' : 'ok') : 'info'}`">
        <span class="pre__glyph">{{ fluffy ? "✓" : "·" }}</span>
        <span class="pre__word rs-mono">{{ fluffy ? "FOUND" : "NONE" }}</span>
        <span class="pre__what">Fluffy Mod Manager</span>
        <span class="pre__val rs-mono">{{
          fluffy
            ? `${fluffy.root} · ${fluffy.installedCount} installed, ${fluffy.unadoptedCount} not adopted`
            : "not on this PC"
        }}</span>
        <RiftButton v-if="fluffy?.unadoptedCount" size="sm" @click="nav.go('world')">Adopt installs</RiftButton>
      </li>
      <li v-for="c in spineChecks" :key="c.id" class="pre__row" :class="`is-${c.level}`">
        <span class="pre__glyph">{{ GLYPH[c.level].glyph }}</span>
        <span class="pre__word rs-mono">{{ GLYPH[c.level].word }}</span>
        <span class="pre__what">{{ c.title }}</span>
        <span class="pre__val">{{ c.detail }}</span>
        <RiftButton v-if="c.action" size="sm" @click="preflight.runCheck(c.action)">{{
          c.action.kind === "reviewParked" ? "Review" : "Fix"
        }}</RiftButton>
      </li>
    </ol>

    <section class="pre__sec">
      <header class="pre__sechead">
        <h2 class="pre__h rs-inscription">LIBRARY ROOTS</h2>
        <RiftButton size="sm" variant="primary" @click="preflight.addRoot()">Add folder…</RiftButton>
        <RiftButton size="sm" :busy="stone.scanning.value" @click="preflight.rescan()">Scan</RiftButton>
      </header>
      <ul class="pre__list">
        <li v-for="r in o?.roots ?? []" :key="r.id" class="pre__row is-plain">
          <span class="pre__what">{{ r.label }}</span>
          <span class="pre__val rs-mono"
            >{{ r.path }}<template v-if="r.dd2ShapedOnly"> · only DD2-shaped archives</template></span
          >
          <RiftButton
            size="sm"
            variant="danger"
            :busy="preflight.busy.value === `root:${r.id}`"
            @click="preflight.removeRoot(r.id)"
            >Remove</RiftButton
          >
        </li>
      </ul>
    </section>

    <section class="pre__sec">
      <header class="pre__sechead">
        <h2 class="pre__h rs-inscription">NEXUS KEY</h2>
      </header>
      <ul class="pre__list">
        <li class="pre__row is-plain">
          <span class="pre__what">Personal API key</span>
          <span class="pre__val rs-mono">{{
            o?.nexus.hasApiKey
              ? "•••••••••••• (stored in Windows Credential Manager)"
              : "none — update checks run without one"
          }}</span>
          <RiftButton size="sm" @click="nav.go('nexus')">{{ o?.nexus.hasApiKey ? "Replace" : "Add" }}</RiftButton>
        </li>
      </ul>
    </section>

    <section class="pre__sec">
      <header class="pre__sechead"><h2 class="pre__h rs-inscription">SETTINGS</h2></header>
      <ul class="pre__list">
        <li class="pre__row is-plain pre__row--veil">
          <span id="veil-label" class="pre__what">Veil screenshots</span>
          <div class="veil" role="radiogroup" aria-labelledby="veil-label">
            <label v-for="v in VEILS" :key="v.id" class="veil__opt" :class="{ 'is-on': settings.veil === v.id }">
              <input
                type="radio"
                name="rift-veil"
                class="veil__radio"
                :value="v.id"
                :checked="settings.veil === v.id"
                @change="setVeil(v.id)"
              />
              <span class="veil__swatch" :class="`veil__swatch--${v.id}`" aria-hidden="true"><i /><i /><i /></span>
              <span class="veil__word">{{ v.label }}</span>
            </label>
          </div>
          <span class="pre__val pre__val--veil">{{ veilLine }}</span>
        </li>
        <li class="pre__row is-plain">
          <label class="pre__toggle">
            <input
              type="checkbox"
              :checked="settings.autoCheckUpdates"
              @change="setAutoCheck(!settings.autoCheckUpdates)"
            />
            <span class="pre__what">Check Nexus on launch</span>
          </label>
          <span class="pre__val">When the last check is older than 12 hours.</span>
        </li>
      </ul>
    </section>
  </main>
</template>

<style scoped>
.pre {
  flex: 1;
  min-width: 0;
  height: 100%;
  overflow-y: auto;
  padding: var(--rs-space-5) var(--rs-space-6) var(--rs-space-12);
  background: var(--rs-hearth);
}
.pre__head {
  padding-bottom: var(--rs-space-4);
  border-bottom: 1px solid var(--rs-edge);
}
.pre__title {
  margin: 0;
  font-size: var(--rs-text-xl);
  line-height: 1.15;
}
.pre__sub {
  margin: 4px 0 0;
  color: var(--rs-muted);
}
.pre__list {
  margin: var(--rs-space-4) 0 0;
  padding: 0;
  list-style: none;
  max-width: 960px;
}
.pre__row {
  display: grid;
  grid-template-columns: 16px 64px 190px minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--rs-space-3);
  min-height: 44px;
  padding: 6px var(--rs-space-3);
  margin-bottom: 2px;
  background: var(--rs-hearthhi);
  border-left: 3px solid transparent;
}
.pre__row.is-plain {
  grid-template-columns: 190px minmax(0, 1fr) auto;
}
.pre__row.is-ok {
  border-left-color: var(--rs-world);
}
.pre__row.is-warn {
  border-left-color: var(--rs-warn);
}
.pre__row.is-error {
  border-left-color: var(--rs-ember);
}
.pre__row.is-info {
  border-left-color: var(--rs-edgehi);
}
.pre__glyph {
  text-align: center;
}
.is-ok .pre__glyph,
.is-ok .pre__word {
  color: var(--rs-world);
}
.is-warn .pre__glyph,
.is-warn .pre__word {
  color: var(--rs-warn);
}
.is-error .pre__glyph,
.is-error .pre__word {
  color: var(--rs-ember);
}
.is-info .pre__glyph,
.is-info .pre__word {
  color: var(--rs-muted);
}
.pre__what {
  color: var(--rs-text);
  font-weight: 500;
}
.pre__val {
  min-width: 0;
  color: var(--rs-muted);
  overflow-wrap: anywhere;
}
.pre__sec {
  margin-top: var(--rs-space-6);
}
.pre__sechead {
  display: flex;
  align-items: center;
  gap: var(--rs-space-3);
}
.pre__h {
  margin: 0;
  font-size: 14px;
  color: var(--rs-muted);
  letter-spacing: 0.1em;
}
/* v0.2 — the veil as three panes of frosted glass: none, one, all three. */
.pre__row--veil {
  grid-template-columns: 190px auto minmax(0, 1fr);
}
.veil {
  display: flex;
  gap: 2px;
}
.veil__opt {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--rs-space-2);
  min-height: 36px;
  padding: 0 var(--rs-space-3);
  border: 1px solid var(--rs-edgehi);
  color: var(--rs-muted);
  cursor: pointer;
  transition:
    color var(--rs-dur-quick),
    border-color var(--rs-dur-quick);
}
.veil__opt:hover,
.veil__opt.is-on {
  color: var(--rs-text);
}
/* chosen the way a NEXUS chip is chosen (riftdeep fill, rift border) plus a
   carved dot: gold stays the colour of things in your world (§4.2, §7.7),
   and colour is never the only carrier (§1) */
.veil__opt.is-on {
  border-color: var(--rs-rift);
  background: var(--rs-riftdeep);
}
.veil__opt::before {
  content: "";
  flex: none;
  width: 8px;
  height: 8px;
  border: 1px solid var(--rs-faint);
  border-radius: 50%;
}
.veil__opt.is-on::before {
  border-color: var(--rs-rift);
  background: var(--rs-rift);
}
.veil__opt:has(:focus-visible) {
  outline: 2px solid var(--rs-rift);
  outline-offset: 2px;
}
.veil__radio {
  position: absolute;
  opacity: 0;
  pointer-events: none;
}
.veil__swatch {
  display: flex;
  gap: 2px;
}
.veil__swatch i {
  width: 8px;
  height: 12px;
  background: var(--rs-rift);
}
.veil__swatch--adult i:nth-child(2),
.veil__swatch--all i {
  background: var(--rs-edgehi);
  box-shadow: inset 0 0 0 1px var(--rs-faint);
}
.veil__word {
  white-space: nowrap;
}
.pre__val--veil {
  grid-column: 2 / -1;
}
.pre__toggle {
  display: flex;
  align-items: center;
  gap: var(--rs-space-2);
  cursor: pointer;
}
.pre__toggle input {
  width: 16px;
  height: 16px;
  accent-color: var(--rs-world);
}
</style>
