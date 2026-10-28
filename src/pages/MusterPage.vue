<script setup lang="ts">
// v0.4 — THE MUSTER (#00015 §17): the party from the player's own saves. The
// whole page is your world (hearth); a hired pawn's sheet is cold. Three
// regions: the pinned page head (inscription, purse, the open slot's line,
// THE RE-CARVING's carved count), the slot list, and the member tab line over
// one member's sheet. The manual backup door copies encrypted files into a
// separate folder; every live save remains read-only.
//
// v0.5 (#00015 §18.8–§18.10): the left column lists characters, with the
// slots that won't open apart; the head's open-slot line is the slot menu,
// with the words that say whether the sheet follows the newest save or is
// pinned to one.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useMuster } from "@/composables/useMuster";
import { useSaveBackup } from "@/composables/useSaveBackup";
import { useNavigation } from "@/composables/useNavigation";
import { useToasts } from "@/composables/useToasts";
import { failureDetails, voice, voicedLine, type RecoveryKind } from "@/services/voice";
import {
  LISTING_SLOTS,
  LOOK_AGAIN,
  MUSTER_FOLDER_STATES,
  MUSTER_INSCRIPTION,
  NAMES_NOTICE,
  READING_PARTY,
  CHARACTERS_HEAD,
  count,
  readAgainWords,
  stateWords,
  whenWords,
} from "@/services/muster";
import { deltaText, membersOf } from "@/services/recarving";
import MusterRoster from "@/components/MusterRoster.vue";
import MusterSlotMenu from "@/components/MusterSlotMenu.vue";
import MusterTabs from "@/components/MusterTabs.vue";
import MusterSheet from "@/components/MusterSheet.vue";
import MusterState from "@/components/MusterState.vue";
import RiftButton from "@/components/RiftButton.vue";

const muster = useMuster();
const backup = useSaveBackup();
const nav = useNavigation();

const roster = ref<InstanceType<typeof MusterRoster> | null>(null);
const tabs = ref<InstanceType<typeof MusterTabs> | null>(null);
const panel = ref<HTMLElement | null>(null);

const list = computed(() => muster.slotsView.value);
const view = computed(() => muster.view.value);
const member = computed(() => muster.activeMember.value);
const marks = computed(() => muster.marks.value);
const party = computed(() => (view.value ? membersOf(view.value) : []));
const cold = computed(() => member.value?.role === "hiredPawn");

const folderState = computed(() => {
  const problem = list.value?.problem;
  return problem ? MUSTER_FOLDER_STATES[problem] : null;
});

/** R17.23: a slot's state takes the tab line's and the sheet's place. */
const slotState = computed(() => muster.viewVoiced.value);
const RAILS = { beingWritten: "warn", damaged: "ember", gone: "faint" } as const;
const slotRail = computed(() => {
  const reason = muster.viewFailure.value?.detail.reason;
  if (muster.viewFailure.value?.kind !== "save") return "ember";
  return reason && reason in RAILS ? RAILS[reason as keyof typeof RAILS] : "none";
});

const purse = computed(() => {
  const v = view.value;
  if (!v) return [];
  const parts: { label: string; value: number; delta: string | null }[] = [];
  if (v.gold !== null) parts.push({ label: "Gold", value: v.gold, delta: deltaText(marks.value.gold) });
  if (v.riftCrystals !== null)
    parts.push({ label: "Rift Crystals", value: v.riftCrystals, delta: deltaText(marks.value.riftCrystals) });
  return parts;
});

/** R18.20: the menu stands only while a character's save is on the sheet. */
const menuFor = computed(() => (muster.picked.value ? muster.shownCharacter.value : null));
const state = computed(() => stateWords(muster.stateMode.value));
/** R18.25: `moved from` replaces `read again` for that read. */
const readAgainShown = computed(
  () =>
    muster.readAgainAt.value !== null &&
    !(menuFor.value && muster.stateMode.value.kind === "following" && muster.stateMode.value.movedFrom),
);

const readingShown = computed(() => muster.readingSlow.value && !view.value && !slotState.value);

async function copyDetails(): Promise<void> {
  const failure = muster.viewFailure.value ?? muster.slotsFailure.value;
  if (!failure) return;
  const text = failureDetails(
    { kind: failure.kind, detail: { ...failure.detail, file: muster.pickedId.value ?? "" } },
    "muster",
  );
  try {
    await navigator.clipboard.writeText(text);
    useToasts().push("rift", "The details are on your clipboard.");
  } catch {
    useToasts().push("warn", "The clipboard refused the details.");
  }
}

async function showInFolder(): Promise<void> {
  const failure = await muster.openSavesFolder();
  if (failure) useToasts().push("ember", voicedLine(voice(failure, "muster")));
}

const DOORS: Partial<Record<RecoveryKind | "lookAgain", () => unknown>> = {
  lookAgain: () => muster.lookAgain(),
  retry: () => (muster.slotsFailure.value ? muster.lookAgain() : muster.readAgain()),
  showInFolder,
  copyDetails,
  pickSlot: () => roster.value?.focusNewestOpen(),
  openPreflight: () => nav.go("preflight"),
};

function door(kind: RecoveryKind | "lookAgain"): void {
  void DOORS[kind]?.();
}

function body(voiced: { voice: string; empathy: string }): string {
  return [voiced.voice, voiced.empathy].filter(Boolean).join(" ");
}

function focusTabs(): void {
  void nextTick(() => tabs.value?.focusActive());
}

function focusSheet(): void {
  panel.value?.focus();
}

function typing(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
}

/** R17.27: `[` / `]` step the member anywhere on the page, focus unchanged. */
function onKeydown(event: KeyboardEvent): void {
  if (event.defaultPrevented || event.ctrlKey || event.metaKey || event.altKey || typing(event.target)) return;
  if (event.key !== "[" && event.key !== "]") return;
  if (muster.step(event.key === "]" ? 1 : -1)) event.preventDefault();
}

/** `5` brought the user here: the focus lands on the picked slot (R17.27). */
watch(
  () => [muster.focusSummoned.value, list.value, muster.pickedId.value] as const,
  async ([summoned, slots]) => {
    if (!summoned || !slots) return;
    await nextTick();
    if (slots.slots.length) roster.value?.focusShown();
    muster.focusSummoned.value = false;
  },
  { immediate: true },
);

onMounted(() => {
  window.addEventListener("keydown", onKeydown);
  void muster.enter();
});
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <main class="muster" aria-labelledby="muster-title">
    <header class="muster__head">
      <div class="muster__headline">
        <h1 id="muster-title" class="muster__title rs-inscription">{{ MUSTER_INSCRIPTION }}</h1>
        <p v-if="purse.length" class="muster__purse">
          <template v-for="(p, i) in purse" :key="p.label"
            ><span v-if="i" class="muster__dim" aria-hidden="true"> · </span
            ><span class="muster__plabel">{{ p.label }}</span
            >{{ " "
            }}<span class="rs-mono muster__pval rc" :class="{ 'is-lit': muster.lit.value && p.delta }">{{
              count(p.value)
            }}</span
            ><span v-if="p.delta" class="rc-mark">{{ p.delta }}</span></template
          >
        </p>
      </div>
      <div class="muster__slotline">
        <div v-if="menuFor && muster.picked.value" class="muster__open">
          <MusterSlotMenu
            :who="menuFor"
            :shown-slot="muster.picked.value"
            :shown-at="view?.savedAt ?? muster.picked.value.savedAt"
            :newer-ids="muster.newerIds.value"
            :lit="muster.slotLit.value"
            @pick="(id) => void muster.pickSlot(id)"
          />
          <span class="muster__rest">
            <span class="rs-mono muster__file muster__file--menu">{{ muster.picked.value.slotId }}</span>
            <!-- the spaces are interpolated: Vue condenses a leading or newline-bearing blank away -->
            <span class="muster__state" :class="{ 'is-warn': state.warn }">{{ ` ${state.words}` }}</span>
            <span v-if="readAgainShown && muster.readAgainAt.value !== null" class="muster__dim">{{
              ` ${readAgainWords(muster.readAgainAt.value)}`
            }}</span>
          </span>
        </div>
        <p v-else-if="muster.picked.value" class="muster__open muster__open--plain">
          <span class="muster__slot">Slot {{ muster.picked.value.slot }}</span>
          <span class="muster__dim"> · </span>
          <span class="rs-mono muster__file">{{ muster.picked.value.slotId }}</span>
          <span class="muster__dim">
            · saved
            <time :datetime="view?.savedAt ?? muster.picked.value.savedAt">{{
              whenWords(view?.savedAt ?? muster.picked.value.savedAt)
            }}</time></span
          >
          <span v-if="muster.readAgainAt.value !== null" class="muster__dim">
            {{ readAgainWords(muster.readAgainAt.value) }}</span
          >
        </p>
        <span v-else />
        <span v-if="marks.count" class="muster__changed rs-mono">{{ marks.count }} CHANGED</span>
      </div>
      <p class="rs-sr-only" aria-live="polite">{{ muster.announcement.value }}</p>
      <div class="muster__backup-tools">
        <RiftButton size="sm" :busy="backup.busy.value" @click="backup.create"> Back up all saves… </RiftButton>
        <span class="muster__dim">All Steam accounts and slots, including system data. Close the game first.</span>
      </div>
      <p v-if="backup.report.value" class="muster__backup-result" role="status">
        Verified backup: {{ backup.report.value.files }} {{ backup.report.value.files === 1 ? "file" : "files" }} across
        {{ backup.report.value.accounts }} {{ backup.report.value.accounts === 1 ? "account" : "accounts" }}.
        <RiftButton size="sm" @click="backup.show">Show backup</RiftButton>
      </p>
      <p v-if="backup.error.value" class="muster__backup-error" role="alert">{{ backup.error.value }}</p>
    </header>

    <MusterState
      v-if="folderState"
      class="muster__pagestate"
      scale="page"
      :inscription="folderState.inscription"
      :body="folderState.body"
      :doors="[{ kind: 'lookAgain', label: LOOK_AGAIN }]"
      @door="door"
    />
    <MusterState
      v-else-if="muster.slotsVoiced.value && !list"
      class="muster__pagestate"
      scale="page"
      :inscription="muster.slotsVoiced.value.inscription"
      :body="body(muster.slotsVoiced.value)"
      :doors="muster.slotsVoiced.value.recovery"
      @door="door"
    />
    <div v-else class="muster__body">
      <!-- a slot's state comes first in the DOM (its doors lead the tab order,
           R17.27) and sits in the sheet's place by grid area -->
      <div v-if="slotState" class="muster__sheetcol muster__statecol">
        <MusterState
          :inscription="slotState.inscription"
          :body="body(slotState)"
          :doors="slotState.recovery"
          :rail="slotRail"
          @door="door"
        />
      </div>

      <section class="muster__slots" aria-labelledby="muster-slots-h">
        <h2 id="muster-slots-h" class="msec rs-inscription">{{ CHARACTERS_HEAD }}</h2>
        <div class="muster__slotscroll">
          <p v-if="!list && muster.listingSlow.value" class="muster__quiet">{{ LISTING_SLOTS }}</p>
          <MusterRoster
            v-if="list"
            ref="roster"
            :characters="muster.characters.value"
            :apart="muster.apart.value"
            :shown="muster.shownCharacter.value"
            :picked-id="muster.pickedId.value"
            :names="list.names"
            :marked="marks.count > 0"
            :gone="muster.goneShown.value"
            @pick-character="(key) => void muster.pickCharacter(key)"
            @pick-apart="(id) => void muster.pickApart(id)"
            @to-tabs="focusTabs"
          />
        </div>
      </section>

      <div v-if="!slotState" class="muster__sheetcol" :class="{ 'is-cold': cold }">
        <template v-if="view && member">
          <MusterTabs
            ref="tabs"
            class="muster__tabs"
            :places="muster.places.value"
            :active="muster.activePlace.value"
            :marks="marks"
            @select="muster.select"
            @to-sheet="focusSheet"
          />
          <div
            id="muster-sheet"
            ref="panel"
            class="muster__sheet"
            role="tabpanel"
            tabindex="0"
            :aria-labelledby="`muster-tab-${muster.activePlace.value}`"
            :aria-busy="muster.reading.value ? 'true' : 'false'"
          >
            <MusterSheet
              :member="member"
              :party="party"
              :names="view.names"
              :marks="marks.members.get(member.memberKey)"
              :lit="muster.lit.value"
            />
          </div>
          <aside v-if="view.names === 'missing'" class="muster__notice" aria-labelledby="muster-names-h">
            <h2 id="muster-names-h" class="msec rs-inscription">{{ NAMES_NOTICE.inscription }}</h2>
            <p v-if="view.namesNote" class="muster__note">{{ view.namesNote }}</p>
            <p class="muster__noteline">{{ NAMES_NOTICE.line }}</p>
            <RiftButton size="sm" @click="nav.go('preflight')">{{ NAMES_NOTICE.door }}</RiftButton>
          </aside>
        </template>
        <MusterState
          v-else-if="readingShown"
          :inscription="READING_PARTY.inscription"
          :body="READING_PARTY.body"
          :doors="[]"
          busy
        />
      </div>
    </div>
  </main>
</template>

<style scoped>
.muster__backup-tools,
.muster__backup-result {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--rs-space-3);
  margin-top: var(--rs-space-3);
  font-size: var(--rs-text-sm);
}
.muster__backup-error {
  color: var(--rs-ember);
  margin-top: var(--rs-space-3);
}
.muster {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
  background: var(--rs-hearth);
}
.muster__head {
  flex: none;
  padding: var(--rs-space-5) var(--rs-space-6) var(--rs-space-3);
  border-bottom: 1px solid var(--rs-edge);
}
.muster__headline,
.muster__slotline {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  gap: var(--rs-space-4);
  min-width: 0;
}
.muster__title {
  margin: 0;
  color: var(--rs-text);
  font-size: var(--rs-text-xl);
  line-height: 1.15;
}
.muster__purse {
  margin: 0;
  white-space: nowrap;
}
.muster__plabel {
  color: var(--rs-muted);
  font-weight: 500;
}
.muster__pval {
  color: var(--rs-text);
  font-weight: 600;
}
.muster__purse {
  --temp: var(--rs-world);
}
.muster__slotline {
  margin-top: 4px;
  min-height: 21px;
}
.muster__open {
  margin: 0;
  min-width: 0;
  white-space: nowrap;
}
/* R18.20: the slot menu's button, then the file and the state words on one line */
.muster__open:not(.muster__open--plain) {
  display: flex;
  align-items: baseline;
  gap: var(--rs-space-3);
}
.muster__open--plain,
.muster__rest {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}
.muster__state {
  color: var(--rs-muted);
}
.muster__state.is-warn {
  color: var(--rs-warn);
}
/* Q18.9's fallback: below 1200 the file name leaves line 2 (the button's
   description still carries it) */
@media (max-width: 1199px) {
  .muster__file--menu {
    display: none;
  }
}
.muster__slot {
  color: var(--rs-text);
  font-weight: 500;
}
.muster__file,
.muster__dim {
  color: var(--rs-faint);
}
.muster__changed {
  flex: none;
  color: var(--rs-world);
  font-weight: 600;
  letter-spacing: 0.12em;
}
.muster__pagestate {
  padding: var(--rs-space-8) var(--rs-space-6);
}
.muster__body {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 248px minmax(0, 1fr);
  grid-template-areas: "slots sheet";
}
.muster__slots {
  grid-area: slots;
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: var(--rs-space-5) var(--rs-space-3) 0 var(--rs-space-4);
  border-right: 1px solid var(--rs-edge);
}
.muster__slotscroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding-bottom: var(--rs-space-6);
}
.muster__quiet {
  margin: 0;
  color: var(--rs-faint);
}
.muster__sheetcol {
  --temp: var(--rs-world);
  --rs-row: var(--rs-hearthhi);
  grid-area: sheet;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  padding: var(--rs-space-6) var(--rs-space-6) 0;
}
.muster__statecol {
  padding-top: var(--rs-space-6);
}
.muster__sheetcol.is-cold {
  --temp: var(--rs-rift);
  --rs-row: var(--rs-misthi);
}
.muster__tabs {
  flex: none;
}
/* the sheet takes the selected tab's ground, so tab and sheet read as one
   slab (R17.10): warm hearthhi, a hired pawn's mist */
.muster__sheet {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--rs-space-5) var(--rs-space-5) var(--rs-space-12);
  background: var(--rs-hearth);
  box-shadow: inset 0 1px 0 var(--rs-edge);
  transition: background-color var(--rs-dur-settle) var(--rs-ease-carve);
}
.is-cold .muster__sheet {
  background: var(--rs-mist);
}
.muster__sheet:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: -2px;
}
/* R17.20: above the tab line on screen, after the sheet in the tab order */
.muster__notice {
  order: -1;
  flex: none;
  margin-bottom: var(--rs-space-4);
  padding: var(--rs-space-3) var(--rs-space-4);
  border-left: 4px solid var(--rs-faint);
  background: var(--rs-hearthhi);
}
.muster__note {
  margin: 0 0 4px;
  color: var(--rs-text);
}
.muster__noteline {
  margin: 0 0 var(--rs-space-3);
  color: var(--rs-muted);
}
</style>
