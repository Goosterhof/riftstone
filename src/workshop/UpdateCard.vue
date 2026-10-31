<script setup lang="ts">
// 0.8 — IN ITS PLACE (#00015 R20.32–R20.34): BRING IT THROUGH's card, with the
// earlier build it replaces. Each block sits on the ground of where its step
// lands (Bold Choice 29): the new zip on cold mist (the mods folder), the swaps
// on warm hearthhi (your game), the put-away on void (beneath the Rift). The
// digest is frozen when the card opens and the press carries it, with the
// earlier build the card showed; the stone refuses either one changed. The
// confirm is World gold when the press reaches your game (a swap), Primary
// when it only lands and puts away (§7.7: the furthest the press reaches).
import { computed, nextTick, onMounted, ref } from "vue";
import type { BringThroughPlan, UpdateResult } from "@/types/workshop";
import RiftButton from "@/components/RiftButton.vue";
import type { RiftFailure } from "@/services/rift";
import { useToasts } from "@/composables/useToasts";
import { useLaunch } from "@/composables/useLaunch";
import { useWorkshop } from "./useWorkshop";
import { quads, size, swapLine, updatedLine } from "./words";

const { plan: opened } = defineProps<{ plan: BringThroughPlan }>();
const emit = defineEmits<{ close: [] }>();

const w = useWorkshop();
const launch = useLaunch();
const plan = ref<BringThroughPlan>(opened);
type Phase = "review" | "updating" | "updated" | "swapRefused" | "bytesChanged" | "planChanged" | "refused";
const phase = ref<Phase>("review");
const said = ref<string | null>(null);
const done = ref<UpdateResult | null>(null);
const card = ref<HTMLElement | null>(null);

const earlier = computed(() => plan.value.earlier);
const swaps = computed(() => earlier.value?.swaps ?? []);
const reachesWorld = computed(() => swaps.value.length > 0);
const carved = computed<string | null>(() => {
  if (phase.value === "updated") return reachesWorld.value ? "IN YOUR WORLD" : "IN THE RIFT";
  if (phase.value === "swapRefused") return "IN THE RIFT";
  if (phase.value === "bytesChanged") return "THE BYTES CHANGED";
  if (phase.value === "planChanged") return "THE PLAN CHANGED";
  return null;
});
const promise = computed(() => {
  const e = earlier.value;
  if (!e) return "The stone will add exactly these bytes to your mods folder, and nothing else.";
  if (!e.swaps.length && e.putsAway) {
    return `The stone will add these bytes to your mods folder, then put ${e.fileName} away. Nothing is installed.`;
  }
  const away = e.putsAway ? `, then put ${e.fileName} away` : "";
  return `The stone will add these bytes to your mods folder and swap ${e.fileName}'s installs for this build's in one step${away}.`;
});

const REFUSED_PHASE: Partial<Record<string, Phase>> = {
  bytesChanged: "bytesChanged",
  earlierChanged: "planChanged",
};

/** The stone's refusal of the press: its sentence, and the state it carves. */
function refusedBy(failure: RiftFailure | null): void {
  said.value = failure?.detail.detail ?? failure?.message ?? null;
  const reason = failure?.kind === "bring" ? failure.detail.reason : "";
  phase.value = REFUSED_PHASE[reason] ?? "refused";
}

async function press(): Promise<void> {
  phase.value = "updating";
  said.value = null;
  const result = await w.updateBuild(plan.value);
  if (!result) {
    refusedBy(w.failure.value);
    return;
  }
  done.value = result;
  if (result.failed) {
    phase.value = "swapRefused";
    return;
  }
  phase.value = "updated";
  useToasts().push(reachesWorld.value ? "world" : "rift", updatedLine(plan.value.artifact, result));
}

/** Review it again: a new card on the bytes and the world as they are now. */
async function again(): Promise<void> {
  const next = await w.planBring(plan.value.artifact);
  if (!next) {
    said.value = w.failure.value?.detail.detail ?? w.failure.value?.message ?? null;
    phase.value = "refused";
    return;
  }
  plan.value = next;
  said.value = null;
  done.value = null;
  phase.value = "review";
}

onMounted(async () => {
  await nextTick();
  card.value?.focus({ preventScroll: false });
});
</script>

<template>
  <section
    ref="card"
    class="update"
    :class="`is-${phase}`"
    tabindex="-1"
    aria-label="Update in its place"
    @keydown.esc.stop="emit('close')"
  >
    <p class="update__inscription rs-inscription">IN ITS PLACE</p>
    <p
      v-if="carved"
      class="update__state rs-inscription"
      :class="{ 'is-world': carved === 'IN YOUR WORLD' }"
      role="status"
    >
      {{ carved }}
    </p>
    <p v-else class="update__promise">{{ promise }}</p>
    <p v-if="launch.running.value && phase === 'review' && reachesWorld" class="update__note update__note--warn">
      <span aria-hidden="true">⚠ </span>Dragon's Dogma 2 is running. The swap lands now and loads at its next launch.
    </p>

    <div class="update__lands">
      <p class="update__row">
        <span class="update__glyph" aria-hidden="true">+</span>
        <span class="update__path">mods\{{ plan.artifact }}</span>
        <span class="update__meta rs-mono">new · {{ size(plan.bytes) }}</span>
      </p>
      <div class="update__tablet rs-mono" :aria-label="`sha256 ${plan.sha256}`" data-digest>
        <span v-for="(q, i) in quads(plan.sha256)" :key="i">{{ q }}</span>
      </div>
    </div>

    <ul v-if="earlier && (earlier.swaps.length || earlier.stays.length)" class="update__world">
      <li v-for="swap in earlier.swaps" :key="swap.fromOptionId" class="update__row" data-swap>
        <span class="update__glyph update__glyph--world" aria-hidden="true">↻</span>
        <span class="update__path">{{ swapLine(swap) }}</span>
      </li>
      <li v-for="stay in earlier.stays" :key="stay.optionId" class="update__row" data-stay>
        <span class="update__glyph" aria-hidden="true">=</span>
        <span class="update__path">{{ stay.name }} stays: this build has no option that is the same.</span>
      </li>
    </ul>

    <p v-if="earlier" class="update__beneath update__row" data-put-away>
      <span class="update__glyph" aria-hidden="true">↧</span>
      <span v-if="earlier.putsAway" class="update__path">{{ earlier.fileName }} to mods (older versions)</span>
      <span v-else class="update__path"
        >{{ earlier.fileName }} stays in your mods folder: one of its options is still installed.</span
      >
    </p>

    <template v-if="phase === 'updated' && done">
      <p v-if="done.putAway.length" class="update__note">
        ↧ {{ done.putAway[0].fileName }} is put away in mods (older versions), beside your mods folder.
      </p>
      <p v-if="done.putAwayFailed" class="update__note update__note--warn">
        {{ done.putAwayFailed }} The update stands; {{ done.earlier }} stays in your mods folder.
      </p>
    </template>
    <p v-else-if="phase === 'swapRefused' && done" class="update__note update__note--ember">
      {{ plan.artifact }} is in your mods folder, but the swap didn't happen: {{ done.failed }} Your game folder is
      exactly as it was: {{ done.earlier }} is still installed, and its zip stays in your mods folder.
    </p>
    <template v-else-if="phase === 'bytesChanged' || phase === 'planChanged'">
      <p class="update__note" :class="phase === 'bytesChanged' ? 'update__note--ember' : 'update__note--warn'">
        {{ said }}
      </p>
      <p class="update__note">Nothing was added or swapped.</p>
    </template>
    <p v-else-if="phase === 'refused'" class="update__note update__note--ember">{{ said }}</p>

    <div class="update__doors">
      <template v-if="phase === 'review' || phase === 'updating'">
        <RiftButton :variant="reachesWorld ? 'world' : 'primary'" size="sm" :busy="phase === 'updating'" @click="press"
          >Update ▶</RiftButton
        >
        <RiftButton size="sm" @click="emit('close')">Cancel</RiftButton>
      </template>
      <template v-else-if="phase === 'bytesChanged' || phase === 'planChanged'">
        <RiftButton variant="primary" size="sm" @click="again">Review it again</RiftButton>
        <RiftButton size="sm" @click="emit('close')">Cancel</RiftButton>
      </template>
      <RiftButton v-else size="sm" @click="emit('close')">Close</RiftButton>
    </div>
  </section>
</template>

<style scoped>
.update {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-2);
  margin-top: var(--rs-space-2);
  padding: var(--rs-space-3);
  border: 1px solid var(--rs-edgehi);
  background: var(--rs-basalt);
  outline-offset: 0;
}
.update__inscription {
  margin: 0;
  color: var(--rs-text);
  font-size: var(--rs-text-md);
  line-height: 1.1;
}
.update__state {
  margin: 0;
  color: var(--rs-ember);
  font-size: var(--rs-text-sm);
}
.update.is-updated .update__state,
.update.is-swapRefused .update__state {
  color: var(--rs-rift);
}
.update__state.is-world {
  color: var(--rs-world);
}
.update.is-planChanged .update__state {
  color: var(--rs-warn);
}
.update__promise,
.update__note {
  margin: 0;
  color: var(--rs-muted);
  font: 400 var(--rs-text-sm) / 1.4 var(--rs-font-sans);
}
.update__note--ember {
  color: var(--rs-ember);
}
.update__note--warn {
  color: var(--rs-warn);
}
/* Bold Choice 29: each block on the ground of where it lands */
.update__lands {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-1);
  padding: var(--rs-space-3);
  background: var(--rs-mist);
}
.update__world {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-1);
  margin: 0;
  padding: var(--rs-space-3);
  list-style: none;
  background: var(--rs-hearthhi);
}
.update__beneath {
  margin: 0;
  padding: var(--rs-space-2) var(--rs-space-3);
  background: var(--rs-void);
  color: var(--rs-muted);
}
.update__row {
  display: flex;
  gap: var(--rs-space-2);
  margin: 0;
  color: var(--rs-text);
  font: 500 var(--rs-text-sm) var(--rs-font-sans);
}
.update__glyph {
  flex: none;
  color: var(--rs-rift);
  font-family: var(--rs-font-mono);
}
.update__glyph--world {
  color: var(--rs-world);
}
.update__path {
  min-width: 0;
  overflow-wrap: anywhere;
}
.update__meta {
  margin-left: auto;
  flex: none;
  color: var(--rs-faint);
  font-size: 12px;
}
.update__tablet {
  display: grid;
  grid-template-columns: repeat(4, max-content);
  gap: 2px var(--rs-space-3);
  padding: var(--rs-space-2) var(--rs-space-3);
  width: max-content;
  background: var(--rs-void);
  box-shadow: inset 0 2px 0 rgba(0, 0, 0, 0.45);
  color: var(--rs-text);
  letter-spacing: 0.06em;
}
.update__doors {
  display: flex;
  flex-wrap: wrap;
  gap: var(--rs-space-2);
}
</style>
