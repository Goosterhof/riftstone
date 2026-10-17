<script setup lang="ts">
// §8.4 — YOUR WORLD, the whole right of the stone warm. The load order (480)
// beside everything else (552): loose/script/root installs, who overrides
// whom, REFramework's proof that the paks loaded, strays and missing files.
// v0.3: every install, a pak in the load order too, has an uninstall door;
// THE RETURN shows its plan before a byte moves, and comes into view when a
// door far from it is pressed.
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { useInstalled } from "@/composables/useInstalled";
import { useStone } from "@/composables/useStone";
import { usePreflight, REFRAMEWORK_URL } from "@/composables/usePreflight";
import { formatBytes, formatDate, kindsPresent, plural } from "@/services/format";
import { missingPackageTitle } from "@/services/voice";
import {
  PARKED_OCCUPIED_LINE,
  parkedMeta,
  parkedPutBackTitle,
  parkedTicket,
  removalWords,
  returnHeading,
} from "@/services/ledger";
import ConflictMap from "@/components/ConflictMap.vue";
import FailureNotice from "@/components/FailureNotice.vue";
import KindBadge from "@/components/KindBadge.vue";
import LoadOrderList from "@/components/LoadOrderList.vue";
import RiftButton from "@/components/RiftButton.vue";
import RiftInscription from "@/components/RiftInscription.vue";

const installed = useInstalled();
const stone = useStone();
const preflight = usePreflight();

const view = computed(() => installed.view.value);
const game = computed(() => stone.overview.value?.game ?? null);
const pakCount = computed(() => installed.pakRows.value.reduce((n, r) => n + r.paks.length, 0));

const pakLoadLine = computed(() => {
  const report = view.value?.pakLoad;
  if (!report) return "No REFramework log yet: launch the game once and the stone can prove which paks loaded.";
  const started = report.logStarted ? `The game last started ${formatDate(report.logStarted)}` : "The last launch";
  const n = report.injected.length;
  if (report.error) return `${started}. REFramework reported: “${report.error}”`;
  return `${started}; REFramework injected ${plural(n, "pak")} from pak_mods/.`;
});

const parkedSection = ref<HTMLElement | null>(null);
const returnPanel = ref<HTMLElement | null>(null);
const parked = computed(() => view.value?.parked ?? []);

/** Smooth only when the user has not asked for less motion: an explicit JS
 *  `behavior` beats the CSS floor's `scroll-behavior: auto !important`. */
function scrollBehavior(): ScrollBehavior {
  return window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth";
}

/** Preflight's "review parked" lands here: wait for the list, then bring it
 *  into view — once. The summons is consumed, so a later ordinary visit to
 *  YOUR WORLD is not pulled down to PARKED again. */
async function revealParked(): Promise<void> {
  installed.consumeParkedSummons();
  if (!view.value) await installed.load();
  await nextTick();
  parkedSection.value?.scrollIntoView?.({ block: "start", behavior: scrollBehavior() });
  parkedSection.value?.focus({ preventScroll: true });
}

watch(installed.parkedSummons, (n) => {
  if (n > 0) void revealParked();
});

/** A door pressed in the load order opens THE RETURN in the other column (or
 *  far below, at the narrow posture): bring the plan into view and hand it
 *  the keyboard, so its Uninstall is the next press. */
watch(
  () => installed.returning.value?.optionId ?? null,
  async (id) => {
    if (!id) return;
    await nextTick();
    returnPanel.value?.scrollIntoView?.({ block: "nearest", behavior: scrollBehavior() });
    returnPanel.value?.focus({ preventScroll: true });
  },
);

onMounted(() => {
  void installed.load().then(() => {
    if (installed.parkedSummons.value > 0) void revealParked();
  });
});

/** Focus inside one parked slip: every act swaps the buttons under the
 *  user's finger, so focus is put where the next press belongs. */
async function focusIn(store: string, selector: string): Promise<void> {
  await nextTick();
  const row = [...(parkedSection.value?.querySelectorAll<HTMLElement>(".park") ?? [])].find(
    (el) => el.dataset.store === store,
  );
  row?.querySelector<HTMLElement>(selector)?.focus();
}

/** Discard asks first; its safe answer, Keep it, takes the focus. */
async function askDiscard(store: string): Promise<void> {
  await installed.discard(store);
  await focusIn(store, ".park__keep");
}

async function keep(store: string): Promise<void> {
  installed.keepParked();
  await focusIn(store, ".park__discard");
}

/** After a row leaves: the next row's Put back, else the section itself. */
async function afterLeave(index: number): Promise<void> {
  await nextTick();
  const next = parked.value.at(Math.min(index, parked.value.length - 1));
  if (next) await focusIn(next.store, ".park__putback");
  else (parkedSection.value ?? document.getElementById("world-right"))?.focus();
}

async function confirmDiscard(store: string, index: number): Promise<void> {
  await installed.discard(store);
  if (parked.value.some((f) => f.store === store)) await focusIn(store, ".park__discard");
  else await afterLeave(index);
}

async function putBack(store: string, index: number): Promise<void> {
  await installed.putBack(store);
  if (parked.value.some((f) => f.store === store)) await focusIn(store, ".park__putback");
  else await afterLeave(index);
}
</script>

<template>
  <main class="home" aria-label="Your World — installed">
    <header class="home__head">
      <div>
        <h1 class="home__title rs-inscription">YOUR WORLD</h1>
        <p class="home__sub rs-mono">
          {{ plural(view?.installs.length ?? 0, "mod")
          }}<template v-if="game"> · build {{ game.buildId ?? "unknown" }}</template>
        </p>
      </div>
    </header>

    <FailureNotice
      v-if="installed.loadFailure.value"
      :voiced="installed.loadFailure.value"
      @recover="installed.load()"
    />

    <div v-if="stone.buildChange.value" class="home__banner">
      <RiftInscription
        size="md"
        tone="warn"
        inscription="YOUR WORLD HAS CHANGED"
        :body="`Dragon's Dogma 2 updated from build ${stone.buildChange.value.recorded} to ${stone.buildChange.value.now} since your mods were installed. Most still work; script mods and REFramework are the first to break.`"
      >
        <RiftButton size="sm" variant="primary" @click="preflight.openUrl(REFRAMEWORK_URL)"
          >Check REFramework</RiftButton
        >
        <RiftButton size="sm" @click="stone.dismissBuildChange()">I've tested it, dismiss</RiftButton>
      </RiftInscription>
    </div>

    <div class="home__split">
      <section class="home__left">
        <header class="home__colhead">
          <h2 class="home__h rs-inscription">THE LOAD ORDER — <span class="home__later">later wins</span></h2>
          <div v-if="installed.orderDirty.value" class="home__acts">
            <RiftButton size="sm" variant="world" :busy="installed.applying.value" @click="installed.applyOrder()"
              >Apply order</RiftButton
            >
            <RiftButton size="sm" @click="installed.resetOrder()">Reset</RiftButton>
          </div>
        </header>
        <LoadOrderList />
        <p v-if="view && pakCount > view.pakCountWarning" class="home__warn">
          ⚠ {{ pakCount }} paks in pak_mods/. Above {{ view.pakCountWarning }}, REFramework's slot table starts to
          overflow.
        </p>

        <h2 class="home__h rs-inscription home__gap">PAK LOAD PROOF</h2>
        <p class="home__line" :class="{ 'is-ember': view?.pakLoad?.error }">{{ pakLoadLine }}</p>
        <ul v-if="view?.pakLoad?.injected.length" class="home__rows">
          <li v-for="p in view.pakLoad.injected" :key="p.fileName" class="home__row home__row--proof rs-mono">
            <span class="home__name">{{ p.fileName }}</span>
            <span class="home__dim">slot {{ p.slot }} · {{ p.nativeName }}</span>
          </li>
        </ul>
      </section>

      <section id="world-right" class="home__right" tabindex="-1" aria-label="Loose, script and root mods">
        <h2 class="home__h rs-inscription">LOOSE · SCRIPT · ROOT</h2>
        <p v-if="!installed.otherRows.value.length" class="home__line home__dim">
          No loose, script or root mods in your game.
        </p>
        <ul class="home__rows">
          <li v-for="r in installed.otherRows.value" :key="r.optionId" class="home__row home__row--install">
            <span class="home__name">{{ r.name }}</span>
            <KindBadge v-for="k in kindsPresent(r.kinds)" :key="k" :kind="k" />
            <span class="rs-mono home__dim">{{ plural(r.fileCount, "file") }}</span>
            <span v-if="r.origin === 'adopted'" class="rs-mono home__dim">adopted</span>
            <span v-if="r.packageMissing" class="rs-mono is-warn" :title="missingPackageTitle(r)">no package</span>
            <RiftButton
              size="sm"
              variant="danger"
              :aria-label="`Uninstall ${r.name}`"
              @click="installed.planReturn(r.optionId)"
              >−</RiftButton
            >
          </li>
        </ul>

        <div
          v-if="installed.returning.value"
          ref="returnPanel"
          class="home__return"
          tabindex="-1"
          :aria-label="`The return of ${installed.returning.value.name}`"
        >
          <h3 class="home__h rs-inscription">THE RETURN — {{ returnHeading(installed.returning.value.removals) }}</h3>
          <p v-if="installed.returning.value.refused" class="home__line is-ember">
            {{ installed.returning.value.refused }}
          </p>
          <ul class="home__rows rs-mono">
            <li v-for="r in installed.returning.value.removals" :key="r.gamePath" class="home__row">
              <span class="home__name">{{ r.gamePath }}</span>
              <span :class="removalWords(r.action).tone === 'back' ? 'is-world' : 'home__dim'"
                >{{ removalWords(r.action).glyph }} {{ removalWords(r.action).verb }}</span
              >
            </li>
          </ul>
          <div class="home__acts">
            <RiftButton
              variant="danger"
              :busy="installed.returnBusy.value"
              :disabled="installed.returning.value.refused !== null"
              @click="installed.confirmReturn()"
              >Uninstall {{ installed.returning.value.name }}</RiftButton
            >
            <RiftButton @click="installed.cancelReturn()">Cancel</RiftButton>
          </div>
        </div>

        <h2 class="home__h rs-inscription home__gap">WHO OVERRIDES WHOM</h2>
        <ConflictMap :conflicts="view?.conflicts ?? []" />

        <template v-if="view?.missing.length">
          <h2 class="home__h rs-inscription home__gap">MISSING</h2>
          <ul class="home__rows rs-mono">
            <li v-for="m in view.missing" :key="m.gamePath" class="home__row">
              <span class="home__name">{{ m.gamePath }}</span
              ><span class="is-warn">{{ m.optionName }}'s file is gone</span>
            </li>
          </ul>
        </template>

        <section v-if="parked.length" ref="parkedSection" class="home__parked" tabindex="-1" aria-label="Parked files">
          <h2 class="home__h rs-inscription home__gap">
            PARKED — <span class="home__later">kept aside, not deleted</span>
          </h2>
          <p class="home__line home__dim">
            Files the stone set aside rather than delete or force back. Each stays here until you put it back or let it
            go.
          </p>
          <ul class="home__rows">
            <li
              v-for="(f, i) in parked"
              :key="f.store"
              class="park"
              :class="`park--${f.ownerKind}`"
              :data-store="f.store"
            >
              <span class="park__tag rs-mono" aria-hidden="true">{{ parkedTicket(f.store, i) }}</span>
              <div class="park__body">
                <span class="park__path rs-mono">{{ f.gamePath }}</span>
                <span class="park__meta">{{ parkedMeta(f) }}</span>
                <span class="park__why">{{ f.reason }}</span>
                <span v-if="f.occupied" class="park__occ rs-mono">{{ PARKED_OCCUPIED_LINE }}</span>
              </div>
              <div
                v-if="installed.discarding.value === f.store"
                class="park__acts"
                role="group"
                :aria-labelledby="`park-ask-${i}`"
              >
                <span :id="`park-ask-${i}`" class="park__ask">Discard for good? The kept copy can't come back.</span>
                <RiftButton
                  size="sm"
                  variant="danger"
                  :busy="installed.parkedBusy.value === f.store"
                  @click="confirmDiscard(f.store, i)"
                  >Discard for good</RiftButton
                >
                <RiftButton size="sm" class="park__keep" @click="keep(f.store)">Keep it</RiftButton>
              </div>
              <div v-else class="park__acts">
                <RiftButton
                  size="sm"
                  variant="world"
                  class="park__putback"
                  :busy="installed.parkedBusy.value === f.store"
                  :disabled="f.size === null"
                  :reason="parkedPutBackTitle(f)"
                  :title="parkedPutBackTitle(f)"
                  @click="putBack(f.store, i)"
                  >Put back</RiftButton
                >
                <RiftButton
                  size="sm"
                  variant="danger"
                  class="park__discard"
                  :aria-label="`Discard the kept copy of ${f.gamePath}`"
                  @click="askDiscard(f.store)"
                  >Discard</RiftButton
                >
              </div>
            </li>
          </ul>
          <p v-if="installed.parkedNote.value" class="home__line park__note" role="status">
            {{ installed.parkedNote.value }}
          </p>
        </section>

        <template v-if="view?.strays.length">
          <h2 class="home__h rs-inscription home__gap">STRAYS</h2>
          <p class="home__line home__dim">
            Files in the game's mod places that no install put there. The stone leaves them alone.
          </p>
          <ul class="home__rows rs-mono">
            <li v-for="s in view.strays" :key="s.gamePath" class="home__row">
              <span class="home__name">{{ s.gamePath }}</span
              ><span class="home__dim">{{ formatBytes(s.size) }}</span>
            </li>
          </ul>
        </template>
      </section>
    </div>
  </main>
</template>

<style scoped>
.home {
  flex: 1;
  min-width: 0;
  height: 100%;
  overflow-y: auto;
  padding: var(--rs-space-5) var(--rs-space-6) var(--rs-space-12);
  background: var(--rs-hearth);
}
.home__head {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  gap: var(--rs-space-4);
  padding-bottom: var(--rs-space-4);
  border-bottom: 1px solid var(--rs-edge);
}
.home__title {
  margin: 0;
  font-size: var(--rs-text-xl);
  line-height: 1.15;
}
.home__sub {
  margin: 4px 0 0;
  color: var(--rs-faint);
}
.home__banner {
  margin-top: var(--rs-space-4);
}
.home__split {
  display: grid;
  grid-template-columns: minmax(0, 480fr) minmax(0, 552fr);
  gap: var(--rs-space-6);
  margin-top: var(--rs-space-5);
}
@media (max-width: 1199px) {
  .home__split {
    grid-template-columns: minmax(0, 1fr);
  }
}
.home__colhead {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--rs-space-3);
  min-height: 32px;
  margin-bottom: var(--rs-space-2);
}
.home__h {
  margin: 0 0 var(--rs-space-2);
  font-size: 14px;
  color: var(--rs-muted);
  letter-spacing: 0.1em;
}
.home__colhead .home__h {
  margin: 0;
}
.home__later {
  font-family: var(--rs-font-sans);
  font-weight: 500;
  letter-spacing: 0;
  text-transform: none;
  color: var(--rs-faint);
}
.home__gap {
  margin-top: var(--rs-space-6);
}
.home__line {
  margin: 0 0 var(--rs-space-3);
  line-height: 1.5;
}
.home__rows {
  margin: 0 0 var(--rs-space-3);
  padding: 0;
  list-style: none;
}
.home__row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 4px var(--rs-space-3);
  min-height: 34px;
  padding: 4px var(--rs-space-3);
  margin-bottom: 2px;
  background: var(--rs-hearthhi);
}
.home__row--install {
  flex-wrap: nowrap;
}
.home__name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.home__dim {
  color: var(--rs-faint);
}
/* PAK LOAD PROOF: the pak's own name is the datum, so it keeps its full width
   and the slot line wraps under it instead of squeezing it to "zrif…". */
.home__row--proof .home__name {
  flex: 1 0 auto;
  max-width: 100%;
}
.home__row--proof .home__dim {
  min-width: 0;
  overflow-wrap: anywhere;
}
.home__acts {
  display: flex;
  gap: var(--rs-space-2);
}
.home__return {
  margin: var(--rs-space-3) 0;
  padding: var(--rs-space-4);
  border-left: 3px solid var(--rs-ember);
  background: rgba(35, 28, 19, 0.6);
}
.home__return:focus {
  outline: none;
}
.home__return:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: 2px;
}
.home__warn {
  margin: var(--rs-space-3) 0 0;
  color: var(--rs-warn);
}
.is-world {
  color: var(--rs-world);
}
.is-warn {
  color: var(--rs-warn);
}
.is-ember {
  color: var(--rs-ember);
}

/* v0.2 — PARKED: a cloakroom of kept files. Each slip carries its store
   number on a torn stub (the ticket you would hand back), its path, whose
   bytes it holds and why it was set aside. Gold "Put back" is a World act:
   it puts something INTO your game (§7.7). */
.home__right:focus,
.home__parked:focus {
  outline: none;
}
.home__parked:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: 4px;
}
.park {
  display: grid;
  grid-template-columns: 52px minmax(0, 1fr);
  gap: var(--rs-space-1) var(--rs-space-3);
  align-items: start;
  margin-bottom: 2px;
  padding: var(--rs-space-2) var(--rs-space-3) var(--rs-space-2) 0;
  background: var(--rs-hearthhi);
}
/* the ticket is the store's seq (#17), never the file name: clipped, so even
   an odd handle cannot spill over the slip's text */
.park__tag {
  align-self: stretch;
  display: block;
  min-width: 0;
  overflow: hidden;
  padding: 2px 4px 0;
  text-align: center;
  text-overflow: ellipsis;
  white-space: nowrap;
  border-right: 1px dashed var(--rs-edgehi);
  color: var(--rs-faint);
  font-size: var(--rs-text-xs);
  letter-spacing: 0.08em;
}
.park--adopted .park__tag {
  color: var(--rs-world);
}
.park--riftstone .park__tag {
  color: var(--rs-rift);
}
.park__body {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.park__path {
  color: var(--rs-text);
  font-size: var(--rs-text-xs);
  overflow-wrap: anywhere;
}
.park__meta,
.park__why {
  color: var(--rs-muted);
}
.park__why {
  font-style: italic;
}
.park__occ {
  color: var(--rs-warn);
  font-size: var(--rs-text-xs);
}
.park__acts {
  grid-column: 2;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--rs-space-2);
  margin-top: var(--rs-space-1);
}
.park__ask {
  color: var(--rs-ember);
}
.park__note {
  margin-top: var(--rs-space-2);
}
</style>
