<script setup lang="ts">
// S3b: one member's whole table, every key (up to 21), then the seven parts
// at the foot (RF.19b). The spare height goes below the last row (P3.12).
import { computed } from "vue";
import type { FrayFight, FrayMember } from "@/types/fray";
import { MIXSAY, everyKey, isWarm, keyLabel, nameOf, nf, pct, roleLabel, roleLine, share, verdictOf } from "../reading";
import FrayMark from "./FrayMark.vue";
import FraySupport from "./FraySupport.vue";
import FrayTypeGrid from "./FrayTypeGrid.vue";

const { member, fight } = defineProps<{ member: FrayMember; fight: FrayFight }>();
const emit = defineEmits<{ close: [] }>();

const keys = computed(() => everyKey(member));
const say = computed(() => MIXSAY[verdictOf(fight.types)]);
const label = computed(() => `${roleLabel(member)}, ${nameOf(member)}, every skill`);
const isBiggest = (id: string): boolean => fight.biggest?.slot === member.slot && fight.biggest.key === id;
</script>

<template>
  <article class="card open" :class="{ warm: isWarm(member) }" :aria-label="label">
    <div class="ohead">
      <span class="mark"><FrayMark :vocation="member.vocation.name" /></span>
      <div class="who">
        <span class="nm"
          ><span class="n ell">{{ nameOf(member) }}</span></span
        >
        <span class="ln2">{{ roleLine(member) }}</span>
      </div>
      <span class="sd mono b6 osd"
        >{{ nf(member.dps ?? 0) }}<span class="per faint">{{ " /s" }}</span></span
      >
      <span class="sp mono b6 osp">{{ ` ${pct(share(member.damage, fight.hAll))}` }}</span>
      <button type="button" class="door close" data-act="close" @click.stop="emit('close')">✕ Close</button>
    </div>
    <div class="cols">
      <span class="k">skill</span><span class="mini" /><span class="c w56">share</span><span class="c w56">hits</span
      ><span class="c w96">biggest</span><span class="c w96">damage</span>
    </div>
    <div v-for="k in keys.slice(0, 21)" :key="k.id" class="tr">
      <span class="k ell" :class="{ id: k.kind === 'action', muted: k.kind === 'unknown' }" :title="keyLabel(k)"
        ><span v-if="k.lastBlow" class="x mono b6">{{ "✕ " }}</span
        >{{ keyLabel(k) }}</span
      >
      <span class="mini" aria-hidden="true"
        ><i class="fill" :style="{ '--s': share(k.damage, member.damage).toFixed(4) }"
      /></span>
      <span class="c p">{{ pct(share(k.damage, member.damage)) }}</span>
      <span class="c h">{{ k.hits }}</span>
      <span class="c g" :class="{ top: isBiggest(k.id) }">{{ isBiggest(k.id) ? "◆ " : "" }}{{ nf(k.biggest) }}</span>
      <span class="c d">{{ nf(k.damage) }}</span>
    </div>
    <div v-if="keys.length > 21" class="tr faint">{{ `+${keys.length - 21} more, under 1% each` }}</div>
    <div v-if="member.noDamageSkills?.length" class="tr faint">
      <FraySupport :casts="member.noDamageSkills" :max="4" />
    </div>
    <div class="foot">
      <div class="fl">
        <b>{{ say.bold }}</b
        >{{ say.rest }}
      </div>
      <FrayTypeGrid :member="member" :types="fight.types" />
    </div>
    <div class="grow" />
  </article>
</template>
