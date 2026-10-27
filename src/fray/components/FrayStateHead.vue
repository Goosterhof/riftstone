<script setup lang="ts">
// S6/S7: the head band becomes the state block, and the places keep the live
// y (RF.21). With a fight kept, its tag stands on line 1.
import { computed } from "vue";
import type { FrayFight } from "@/types/fray";
import { ageText, agoWords } from "../reading";
import type { FrayDoor, FraySleep } from "../stage";

interface Copy {
  size: "big" | "mid";
  ins: string;
  kept: string;
  fresh: string | null;
  doors: FrayDoor[];
  warn: boolean;
}

/** The silent eye's copy when the feed knows when the game started. */
const silentSince = (s: number): string =>
  `The game started ${ageText(s)} ago and the eye hasn't spoken. If the game just updated, the eye may not fit this version yet.`;

const COPY: Record<FraySleep, Copy> = {
  asleep: {
    size: "big",
    ins: "THE WORLD SLEEPS",
    kept: "Dragon's Dogma 2 isn't running. The last fight stands below as it was set; the stone watches the next one here.",
    fresh:
      "Dragon's Dogma 2 isn't running. When it starts, the stone watches the party here: who deals what, skill by skill.",
    doors: ["Open the Riftstone"],
    warn: false,
  },
  norefw: {
    size: "mid",
    ins: "THE STONE CAN'T SEE THE FIGHT",
    kept: "The stone reads a fight through REFramework, and REFramework isn't in the game folder. Install it from the Rift like any other mod, then start the game again. Nothing in the game was changed.",
    fresh: null,
    doors: ["Open Preflight"],
    warn: false,
  },
  noeye: {
    size: "mid",
    ins: "THE EYE ISN'T SET",
    kept: "The stone's eye, a small REFramework script, isn't among the game's scripts. Preflight sets it in place through the journal; it only watches, and it changes nothing in the game.",
    fresh: null,
    doors: ["Open Preflight"],
    warn: false,
  },
  done: {
    size: "mid",
    ins: "THE EYE HAS CLOSED",
    kept: "The eye watches a set stretch of each run of the game, and this run's is over: what it saw stands below. Start the game again and it watches the next one.",
    fresh:
      "The eye watches a set stretch of each run of the game, and this run's is over. Start the game again and it watches the next one.",
    doors: [],
    warn: false,
  },
  shut: {
    size: "mid",
    ins: "THE EYE IS SHUT",
    kept: "The eye is in place but didn't start: a hook found nothing to hold on this version. It hooked nothing, so the game ran as it always does. Start the game again; if the eye stays shut, copy the details for the stone's keeper.",
    fresh: null,
    doors: ["Open Preflight", "Copy details"],
    warn: true,
  },
  silent: {
    size: "mid",
    ins: "THE GAME RUNS · THE EYE SEES NOTHING",
    kept: "The game is running and the eye hasn't spoken. If the game just updated, the eye may not fit this version yet.",
    fresh: null,
    doors: ["Open Preflight"],
    warn: true,
  },
};

const {
  kind,
  kept,
  minutesAgo,
  startedAgoS = null,
} = defineProps<{
  kind: FraySleep;
  kept: FrayFight | null;
  minutesAgo: number | null;
  /** Seconds since this run of the game started (the silent eye's copy), when the feed knows. */
  startedAgoS?: number | null;
}>();
const emit = defineEmits<{ door: [door: FrayDoor] }>();

const copy = computed(() => COPY[kind]);
const body = computed(() => {
  if (kind === "silent" && startedAgoS !== null) return silentSince(startedAgoS);
  return kept ? copy.value.kept : (copy.value.fresh ?? copy.value.kept);
});
const tag = computed(() => {
  if (!kept) return null;
  return minutesAgo === null ? `FIGHT ${kept.n}` : `FIGHT ${kept.n} · ${agoWords(minutesAgo)}`;
});
</script>

<template>
  <header class="shead" :class="{ warnrail: copy.warn }">
    <div class="sl1">
      <span :class="copy.size">{{ copy.ins }}</span
      ><span v-if="tag" class="kept">{{ tag }}</span>
    </div>
    <div class="body">{{ body }}</div>
    <div class="dl">
      <button v-for="d in copy.doors" :key="d" type="button" class="door" data-act="door" @click="emit('door', d)">
        {{ d }}
      </button>
    </div>
  </header>
</template>
