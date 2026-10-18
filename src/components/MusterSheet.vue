<script setup lang="ts">
// One member's sheet (#00015 R17.9–R17.17): the head (name in its own case,
// the level the one large number, the pawn's traits, a hired pawn's creator),
// then GEAR, GAINS FROM LEVELS, VOCATIONS, AUGMENTS and SKILLS. A hired
// pawn's sheet is cold (mist): it lives in another Arisen's world.
import { computed } from "vue";
import type { MusterMember } from "@/types/rift";
import { ROLE_WORD, count, creatorLead, namedText, personName } from "@/services/muster";
import { deltaText, wasWords, type MemberMarks } from "@/services/recarving";
import MusterNamed from "./MusterNamed.vue";
import MusterGear from "./MusterGear.vue";
import MusterGains from "./MusterGains.vue";

const { member, party, names, marks, lit } = defineProps<{
  member: MusterMember;
  party: MusterMember[];
  names: "game" | "missing";
  marks: MemberMarks | undefined;
  lit: boolean;
}>();

const m = computed<MemberMarks>(() => marks ?? { vocations: new Map(), gains: {}, gear: new Map() });

/** R17.13: the current vocation leads its row with ◆ and ends `current`; it
 *  keeps its place in the contract's order. Absent from the list, it comes last. */
const vocations = computed(() => {
  const rows = member.vocations.map((v) => ({
    named: v.vocation,
    experience: v.experience as number | null,
    current: v.vocation.id === member.vocation.id,
  }));
  if (!rows.some((r) => r.current)) rows.push({ named: member.vocation, experience: null, current: true });
  return rows;
});
</script>

<template>
  <div class="sheet__head">
    <div class="sheet__title">
      <h2 class="sheet__name" :class="{ 'is-nameless': member.name === null }">{{ personName(member) }}</h2>
      <span class="sheet__lv"
        ><span class="sheet__lvword rs-mono">Lv</span>
        <span class="sheet__level rc" :class="{ 'is-lit': lit && m.level }">{{ member.level }}</span>
        <span v-if="m.level" class="rc-mark rs-mono">{{ deltaText(m.level) }}</span></span
      >
    </div>
    <p class="sheet__line">
      {{ ROLE_WORD[member.role] }} ·
      <MusterNamed :named="member.vocation" kind="vocation" :names="names" :mark="m.vocation" :lit="lit" />
      ·
      <span class="rc" :class="{ 'is-lit': lit && m.experience }">{{ count(member.experience) }}</span> experience<span
        v-if="m.experience"
        class="rc-mark rs-mono"
      >
        {{ deltaText(m.experience) }}</span
      >
    </p>
    <p v-if="member.pawn" class="sheet__line sheet__traits">
      <span class="sheet__tlabel">Inclination</span>
      <MusterNamed
        v-if="member.pawn.inclination"
        :named="member.pawn.inclination"
        kind="inclination"
        :names="names"
        :mark="m.inclination"
        :lit="lit"
      />
      <span v-else class="sheet__none">none</span>
      <span class="sheet__sep" aria-hidden="true">·</span>
      <span class="sheet__tlabel">Specialization</span>
      <MusterNamed
        v-if="member.pawn.specialization"
        :named="member.pawn.specialization"
        kind="specialization"
        :names="names"
        :mark="m.specialization"
        :lit="lit"
      />
      <span v-else class="sheet__none">none</span>
    </p>
    <p v-if="member.role === 'hiredPawn'" class="sheet__line sheet__creator">
      {{ creatorLead()
      }}<template v-if="member.creator">
        · their Arisen is a <span class="rs-mono sheet__clv">Lv {{ member.creator.level }}</span>
        {{ namedText(member.creator.vocation, "vocation") }}</template
      >
    </p>
  </div>

  <div class="sheet__grid">
    <div class="sheet__col">
      <MusterGear :gear="member.gear" :names="names" :marks="m.gear" :lit="lit" />
      <section class="voc" aria-labelledby="muster-voc-h">
        <h3 id="muster-voc-h" class="msec rs-inscription">VOCATIONS</h3>
        <ul class="voc__rows">
          <li v-for="v in vocations" :key="v.named.id" class="voc__row" :class="{ 'is-current': v.current }">
            <span class="voc__glyph" aria-hidden="true">{{ v.current ? "◆" : "" }}</span>
            <MusterNamed class="voc__name" :named="v.named" kind="vocation" :names="names" />
            <span v-if="v.experience !== null" class="voc__exp rs-mono"
              ><span class="rc" :class="{ 'is-lit': lit && m.vocations.has(v.named.id) }">{{
                count(v.experience)
              }}</span>
              experience<span v-if="m.vocations.has(v.named.id)" class="rc-mark">
                {{ deltaText(m.vocations.get(v.named.id)) }}</span
              ></span
            >
            <span v-else class="voc__exp voc__none">no experience yet</span>
            <span v-if="v.current" class="voc__current">· current</span>
          </li>
        </ul>
      </section>
    </div>
    <div class="sheet__col">
      <MusterGains :member="member" :party="party" :marks="m.gains" :lit="lit" />
      <div class="sheet__loadout">
        <section aria-labelledby="muster-aug-h">
          <h3 id="muster-aug-h" class="msec rs-inscription">AUGMENTS</h3>
          <p v-if="m.augments" class="loadout__changed">
            <span class="loadout__dot" aria-hidden="true">•</span
            ><span class="rs-sr-only">{{ wasWords(m.augments) }}</span>
          </p>
          <ol v-if="member.augments.length" class="loadout" :class="{ 'rc is-lit': lit && m.augments }">
            <li v-for="(a, i) in member.augments" :key="`${a.id}-${i}`" class="loadout__row">
              <span class="loadout__n rs-mono">{{ i + 1 }}</span>
              <MusterNamed :named="a" kind="augment" :names="names" />
            </li>
          </ol>
          <p v-else class="loadout__none">No augments set.</p>
        </section>
        <section aria-labelledby="muster-skill-h">
          <h3 id="muster-skill-h" class="msec rs-inscription">
            SKILLS — <span class="msec__data">{{ namedText(member.vocation, "vocation") }}</span>
          </h3>
          <p v-if="m.skills" class="loadout__changed">
            <span class="loadout__dot" aria-hidden="true">•</span
            ><span class="rs-sr-only">{{ wasWords(m.skills) }}</span>
          </p>
          <ol v-if="member.skills.length" class="loadout" :class="{ 'rc is-lit': lit && m.skills }">
            <li v-for="(s, i) in member.skills" :key="`${s.id}-${i}`" class="loadout__row">
              <span class="loadout__n rs-mono">{{ i + 1 }}</span>
              <MusterNamed :named="s" kind="skill" :names="names" />
            </li>
          </ol>
          <p v-else class="loadout__none">No skills set.</p>
        </section>
      </div>
    </div>
  </div>
</template>

<style scoped>
.sheet__head {
  padding: 2px 0 2px var(--rs-space-4);
  border-left: 4px solid var(--temp);
}
.sheet__title {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  gap: var(--rs-space-4);
}
.sheet__name {
  margin: 0;
  min-width: 0;
  overflow: hidden;
  color: var(--rs-text);
  font: 700 var(--rs-text-lg) / 1.2 var(--rs-font-sans);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sheet__name.is-nameless {
  color: var(--rs-muted);
  font-weight: 400;
}
.sheet__lv {
  flex: none;
  display: inline-flex;
  align-items: baseline;
  gap: 6px;
}
.sheet__lvword {
  color: var(--rs-muted);
  font-size: var(--rs-text-xs);
  font-weight: 600;
}
.sheet__level {
  color: var(--rs-text);
  font: 700 var(--rs-text-lg) / 1 var(--rs-font-sans);
}
.sheet__line {
  margin: 2px 0 0;
  color: var(--rs-muted);
}
.sheet__line :deep(.named) {
  display: inline;
}
.sheet__traits {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 6px;
}
.sheet__traits :deep(.named) {
  color: var(--rs-text);
}
.sheet__tlabel {
  color: var(--rs-faint);
  font-weight: 500;
}
.sheet__none {
  color: var(--rs-faint);
}
.sheet__sep {
  color: var(--rs-faint);
}
.sheet__clv {
  font-size: var(--rs-text-xs);
  font-weight: 600;
}
.sheet__grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: var(--rs-space-6);
  margin-top: var(--rs-space-5);
}
@media (max-width: 1199px) {
  .sheet__grid {
    grid-template-columns: minmax(0, 1fr);
  }
  /* R17.4's one-column order: GEAR, GAINS, VOCATIONS, AUGMENTS, SKILLS */
  .sheet__col {
    display: contents;
  }
  .sheet__grid > .sheet__col:first-child > .voc {
    order: 3;
  }
  .sheet__grid > .sheet__col:last-child > .gains {
    order: 2;
  }
  .sheet__grid > .sheet__col:last-child > .sheet__loadout {
    order: 4;
  }
}
.sheet__col {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-6);
  min-width: 0;
}
.sheet__loadout {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: var(--rs-space-4);
}
@media (max-width: 1199px) {
  .sheet__loadout {
    grid-template-columns: minmax(0, 1fr);
  }
}
.voc__rows,
.loadout {
  margin: 0;
  padding: 0;
  list-style: none;
}
.voc__row {
  display: flex;
  align-items: baseline;
  gap: var(--rs-space-2);
  min-height: 28px;
}
.voc__glyph {
  flex: none;
  width: 12px;
  color: var(--temp);
}
.voc__name {
  color: var(--rs-text);
  font-weight: 500;
}
.voc__exp {
  color: var(--rs-faint);
  font-size: var(--rs-text-xs);
}
.voc__none {
  font-family: var(--rs-font-sans);
  font-size: var(--rs-text-sm);
}
.voc__current {
  color: var(--rs-faint);
}
.loadout__row {
  display: flex;
  align-items: baseline;
  gap: var(--rs-space-2);
  min-height: 28px;
  min-width: 0;
  color: var(--rs-text);
}
.loadout__n {
  flex: none;
  width: 14px;
  color: var(--rs-faint);
  font-size: var(--rs-text-xs);
}
.loadout__none {
  margin: 0;
  color: var(--rs-faint);
}
.loadout__changed {
  margin: 0;
  color: var(--temp);
}
.msec__data {
  font: 500 var(--rs-text-sm) var(--rs-font-sans);
  letter-spacing: 0;
  text-transform: none;
}
</style>
