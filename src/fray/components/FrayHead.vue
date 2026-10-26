<script setup lang="ts">
// The head band, 128 in every state (P2.1): line 1 (what and how long), line 2
// (PARTY and the axis named once), the vein, and the legend line, which in the
// record carries the Skills / Damage types switch and, browsing, the doors.
import { computed } from "vue";
import type { FrayFight } from "@/types/fray";
import {
  MIXSAY,
  MIXSAY_SHORT,
  ageText,
  agoWords,
  approximate,
  clock,
  foeLine,
  hitsOf,
  nf,
  perSecond,
  verdictOf,
  type Saying,
} from "../reading";
import FrayVein from "./FrayVein.vue";

const { fight, mode, browse, sub, openSlot, staleAgeS, lastBeatAt, minutesAgo } = defineProps<{
  fight: FrayFight;
  mode: "live" | "stale" | "record";
  browse?: boolean;
  sub?: "skills" | "types";
  openSlot?: number | null;
  /** Stale: the age of the last heartbeat (s) and its wall clock. */
  staleAgeS?: number | null;
  lastBeatAt?: string;
  /** Record: minutes since it settled, shown from one minute on. */
  minutesAgo?: number | null;
}>();
const emit = defineEmits<{ sub: [sub: "skills" | "types"]; older: []; newer: [] }>();

const AP_LONG: Record<string, string> = {
  unqualified: " who dealt each hit is the stone's reading, not yet checked",
  inconclusive: " the check of who dealt each hit found nothing to compare",
};
const AP_SHORT = " credit not yet checked";

const ap = computed(() => approximate(fight));
const verdict = computed(() => verdictOf(fight.types));
const asleep = computed(() => openSlot !== null && openSlot !== undefined);
const typesSay = computed(() => mode === "record" && sub === "types" && !asleep.value);
const saying = computed<Saying>(() => (ap.value || browse ? MIXSAY_SHORT : MIXSAY)[verdict.value]);
const partyRate = computed(() => perSecond(fight.hParty, fight.durationS));
const ago = computed(() =>
  !browse && minutesAgo !== null && minutesAgo !== undefined && minutesAgo >= 1 ? agoWords(minutesAgo) : null,
);
const recordTail = computed(() => {
  const lead = ap.value || browse ? "" : "per second, pauses left out";
  const sep = ap.value || !browse ? " · " : "";
  return `${lead}${sep}${nf(hitsOf(fight))} hits · ◆ biggest hit · ✕ last blow`;
});
const liveSay = computed(() => (ap.value ? (AP_LONG[fight.attribution] ?? AP_SHORT) : null));
</script>

<template>
  <header class="head">
    <div v-if="mode === 'stale'" class="l1 stl1">
      <span class="cz ins ell">THE GAME RUNS · THE EYE SEES NOTHING</span>
      <span class="clock"
        >{{ `${clock(fight.durationS)} · `
        }}<span class="agew"
          ><span class="age">{{ ageText(staleAgeS ?? 0) }}</span
          >{{ " ago" }}</span
        ></span
      >
    </div>
    <div v-else class="l1">
      <span class="cz ins" :class="{ set: mode === 'record' }">{{
        mode === "record" ? `${browse ? `FIGHT ${fight.n} · ` : ""}SET IN STONE` : "THE FRAY"
      }}</span>
      <span class="foe ell">{{ foeLine(fight) }}</span>
      <span v-if="mode === 'live' && fight.paused" class="cz ins">PAUSED</span>
      <span v-else class="clock"
        >{{ mode === "record" ? `${clock(fight.durationS)} · ${fight.endedAt ?? "—"}` : clock(fight.durationS)
        }}<span v-if="fight.gaps > 0" class="gapm"
          >{{ " · " }}<span class="n">{{ fight.gaps }}</span
          >{{ " records skipped" }}</span
        ><span v-if="mode === 'record' && ago" class="faint agom">{{ ` · ${ago}` }}</span></span
      >
    </div>
    <div class="l2">
      <span class="cz party">PARTY</span>
      <span v-if="!fight.partyFormed" class="pdps na">—</span>
      <template v-else
        ><span class="pdps">{{ nf(partyRate) }}</span
        ><span class="per">/s</span></template
      >
      <span v-if="fight.hAll > 0" class="tot"
        ><span>% of</span>{{ ` ${nf(fight.hAll)} ` }}<span>damage from every source</span></span
      >
      <span v-else class="tot" />
    </div>
    <FrayVein :fight="fight" :mode="mode === 'record' ? 'carved' : mode === 'stale' ? 'stale' : 'lit'" />
    <div class="legend">
      <template v-if="mode === 'record' && browse">
        <button type="button" class="door" data-act="older" @click="emit('older')">◀ Older</button>
        <button type="button" class="door" data-act="newer" @click="emit('newer')">Newer ▶</button>
      </template>
      <span v-if="mode === 'stale'" class="say ell stl"
        ><template v-if="ap"
          ><span class="ap"><span class="n">≈</span>{{ AP_SHORT }}</span
          >{{ ` · nothing from the eye since ${lastBeatAt}, not live` }}</template
        ><template v-else>{{
          `Nothing from the eye since ${lastBeatAt}. The numbers stood then. They are not live.`
        }}</template></span
      >
      <span v-else-if="mode === 'live'" class="say ell"
        ><span v-if="liveSay" class="ap"><span class="n">≈</span>{{ liveSay }}</span
        ><template v-else>per second, pauses left out</template></span
      >
      <span v-else-if="typesSay" class="say ell"
        ><template v-if="ap"
          ><span class="ap"><span class="n">≈</span>{{ AP_SHORT }}</span
          >{{ " · " }}</template
        ><b v-if="saying.bold">{{ saying.bold }}</b
        >{{ saying.rest }}</span
      >
      <span v-else class="say ell"
        ><span v-if="ap" class="ap"><span class="n">≈</span>{{ AP_SHORT }}</span
        >{{ recordTail }}</span
      >
      <span v-if="mode === 'record'" class="switch" :class="{ asleep }" role="group" aria-label="Breakdown">
        <button
          type="button"
          :aria-pressed="sub !== 'types'"
          :aria-disabled="asleep ? 'true' : undefined"
          data-act="sub"
          @click="!asleep && emit('sub', 'skills')"
        >
          Skills
        </button>
        <button
          type="button"
          :aria-pressed="sub === 'types'"
          :aria-disabled="asleep ? 'true' : undefined"
          data-act="sub"
          @click="!asleep && emit('sub', 'types')"
        >
          Damage types
        </button>
      </span>
    </div>
  </header>
</template>
