<script setup lang="ts">
// One place of the party (§19.4–§19.8). Four kinds of the same card: live (the
// beat's skill block, nothing takes focus), record (four close rows or the
// seven parts, and a door to S3b), strip (the other three while S3b is open),
// and idle/save (names only, no figure). A null member is an empty place.
import { computed } from "vue";
import type { FrayFight, FrayMember } from "@/types/fray";
import {
  approximate,
  blockOf,
  isWarm,
  nameOf,
  nf,
  pct,
  rateText,
  roleLabel,
  roleLine,
  share,
  type SkillBlock,
} from "../reading";
import FrayLintel from "./FrayLintel.vue";
import FrayMark from "./FrayMark.vue";
import FrayRemainder from "./FrayRemainder.vue";
import FrayShare from "./FrayShare.vue";
import FraySupport from "./FraySupport.vue";
import FrayTypeGrid from "./FrayTypeGrid.vue";

export type FrayCardKind = "live" | "record" | "strip" | "idle" | "save";

const { member, fight, kind, block, sub, door, isNew } = defineProps<{
  member: FrayMember | null;
  fight: FrayFight | null;
  kind: FrayCardKind;
  /** The live beat's reading (live only). */
  block?: SkillBlock | null;
  sub?: "skills" | "types";
  /** The record's card is a door to S3b. */
  door?: boolean;
  isNew?: boolean;
}>();
const emit = defineEmits<{ open: [slot: number] }>();

const warm = computed(() => (member ? isWarm(member) : false));
const name = computed(() => (member ? nameOf(member) : ""));
const roleText = computed(() => (member ? roleLine(member) : ""));
const label = computed(() => (member ? `${roleLabel(member)}, ${name.value}` : ""));
const rate = computed(() => member?.dps ?? null);
const approx = computed(() => (fight ? approximate(fight) : false));
const record = computed(() => kind === "record");
const figures = computed(() => fight !== null && (kind === "live" || record.value));
const lowHp = computed(() => {
  const hp = member?.hp ?? null;
  return !member?.down && hp !== null && hp > 0 && hp < 0.25;
});
const liveBlock = computed(() => (member ? (block ?? blockOf(member, 3)) : null));
const recBlock = computed(() => (member ? blockOf(member, 4) : null));
const livePad = computed(() => {
  const b = liveBlock.value;
  if (!b) return 0;
  return Math.max(0, 2 - (b.support ? 1 : 0) - (b.zero ? 1 : 0) - b.lead.length);
});
const recPad = computed(() => {
  const b = recBlock.value;
  if (!b) return 0;
  return Math.max(0, 3 - (b.support ? 1 : 0) - (b.zero ? 1 : 0) - b.lead.length);
});
const isBiggest = (id: string): boolean => fight?.biggest?.slot === member?.slot && fight?.biggest?.key === id;
const isDoor = computed(() => kind === "strip" || (record.value && door === true));

function open(): void {
  if (isDoor.value && member) emit("open", member.slot);
}
</script>

<template>
  <article v-if="!member" class="card empty" aria-label="An empty place">
    <span class="none">No pawn here</span>
  </article>
  <article
    v-else-if="kind === 'strip' && fight"
    class="card strip"
    :class="{ warm }"
    role="button"
    tabindex="0"
    data-act="open"
    :data-slot="member.slot"
    :aria-label="label"
    @click="open"
    @keydown.enter="open"
  >
    <span class="mark"><FrayMark :vocation="member.vocation.name" /></span>
    <div class="who">
      <span class="n ell">{{ name }}</span
      ><span class="ln2 ell">{{ roleText }}</span>
    </div>
    <span class="sd"
      >{{ rateText(rate) }}<span class="per">{{ " /s" }}</span></span
    >
    <span class="sp">{{ pct(share(member.damage, fight.hAll)) }}</span>
  </article>
  <article
    v-else
    class="card"
    :class="{ warm }"
    :role="isDoor ? 'button' : undefined"
    :tabindex="isDoor ? 0 : undefined"
    :data-act="isDoor ? 'open' : undefined"
    :data-slot="member.slot"
    :aria-label="label"
    @click="open"
    @keydown.enter="open"
  >
    <FrayLintel :member="member" :record="kind === 'record' || kind === 'save'" />
    <div class="rowA">
      <span class="mark"><FrayMark :vocation="member.vocation.name" /></span>
      <div class="who">
        <div class="nm">
          <span class="n ell" :class="{ null: member.name === null }">{{ name }}</span>
          <span v-if="member.down" class="hpw">DOWN</span>
          <span v-else-if="lowHp" class="hpw">low</span>
        </div>
        <div class="ln2 ell">
          {{ roleText }}<span v-if="isNew" class="faint">{{ " · new" }}</span
          ><span v-if="member.left" class="faint">{{ " · left" }}</span>
        </div>
      </div>
      <template v-if="figures && fight">
        <data v-if="!fight.partyFormed || rate === null" class="dps" value="" aria-label="damage per second not known"
          >—</data
        >
        <data
          v-else
          class="dps"
          :value="String(Math.round(rate))"
          :aria-label="`${rateText(rate)} damage per second, pauses left out`"
          ><span v-if="approx" class="approx">≈</span>{{ rateText(rate) }}<span class="per">/s</span></data
        >
      </template>
    </div>
    <template v-if="figures && fight">
      <div v-if="!fight.partyFormed" class="share">
        <span class="bar" aria-hidden="true" /><span class="lab"><span class="pc">—</span></span>
      </div>
      <FrayShare v-else :part="member.damage" :whole="fight.hAll" :approx="approx" />

      <div v-if="fight.skills === 'unavailable'" class="rows" :class="{ rec: record }">
        <div class="sk na">Skills: the eye can't tell them apart on this game version</div>
      </div>
      <FrayTypeGrid v-else-if="record && sub === 'types'" :member="member" :types="fight.types" />
      <div v-else-if="record && recBlock" class="rows rec">
        <div v-if="recBlock.support" class="rr blk"><FraySupport :casts="recBlock.support" :max="3" record /></div>
        <div v-if="recBlock.zero" class="rr blk">No damaging hit this fight</div>
        <div v-for="k in recBlock.lead" :key="k.id" class="rr">
          <span class="k ell" :class="{ id: k.kind !== 'skill' }"
            ><span v-if="k.lastBlow" class="x">{{ "✕ " }}</span
            >{{ k.label }}</span
          >
          <span class="mini" aria-hidden="true"><i class="fill" :style="{ '--s': k.s.toFixed(4) }" /></span>
          <span class="p">{{ pct(k.s) }}</span>
          <span class="h"
            ><b>{{ k.hits }}</b
            >{{ k.hits === 1 ? " hit" : " hits" }}</span
          >
          <span class="g"
            >{{ "biggest "
            }}<b :class="{ top: isBiggest(k.id) }">{{ isBiggest(k.id) ? "◆ " : "" }}{{ nf(k.biggest) }}</b></span
          >
        </div>
        <div v-for="i in recPad" :key="`pad${i}`" class="rr" />
        <div class="rr rem"><FrayRemainder :rem="recBlock.rem" /></div>
      </div>
      <div v-else-if="!fight.partyFormed" class="rows">
        <div class="sk na">The stone can't tie hits to this member here</div>
      </div>
      <div v-else-if="liveBlock" class="rows">
        <div v-if="liveBlock.support" class="sk sup ell"><FraySupport :casts="liveBlock.support" :max="2" /></div>
        <div v-if="liveBlock.zero" class="sk zero">No damaging hit yet</div>
        <div v-for="k in liveBlock.lead" :key="k.id" class="sk">
          <span class="k ell" :class="{ id: k.kind !== 'skill' }">{{ k.label }}</span>
          <span class="mini" aria-hidden="true"><i class="fill" :style="{ '--s': k.s.toFixed(4) }" /></span>
          <span class="p">{{ pct(k.s) }}</span>
        </div>
        <div v-for="i in livePad" :key="`pad${i}`" class="sk" />
        <div class="sk rem"><FrayRemainder :rem="liveBlock.rem" /></div>
      </div>
    </template>
  </article>
</template>
