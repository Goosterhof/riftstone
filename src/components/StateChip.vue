<script setup lang="ts">
// §7.5 — state chips. Filled signals carry --rs-ongold text; NOT INSTALLED is
// only outlined: the Rift is the default, it should not shout.
import type { ChipState } from "@/services/chips";

const { state } = defineProps<{ state: ChipState }>();

const LABEL: Record<ChipState, string> = {
  installed: "INSTALLED",
  notInstalled: "NOT INSTALLED",
  update: "UPDATE",
  conflict: "CONFLICT",
  switchOff: "SWITCH OFF",
  menu: "MENU",
};
</script>

<template>
  <span class="rs-chip" :class="`rs-chip--${state}`">{{ LABEL[state] }}</span>
</template>

<style scoped>
.rs-chip {
  display: inline-block;
  padding: 2px 6px;
  border-radius: var(--rs-radius-chip);
  font: 600 var(--rs-text-xs) / 1.3 var(--rs-font-mono);
  letter-spacing: 0.04em;
  white-space: nowrap;
  color: var(--rs-ongold);
  transition:
    background-color var(--rs-dur-quick),
    color var(--rs-dur-quick);
}
.rs-chip--installed {
  background: var(--rs-world);
}
.rs-chip--update {
  background: var(--rs-verdant);
}
.rs-chip--conflict {
  background: var(--rs-ember);
}
.rs-chip--switchOff {
  background: var(--rs-warn);
}
.rs-chip--notInstalled {
  border: 1px solid var(--rs-rift);
  color: var(--rs-rift);
  padding: 1px 5px;
}
.rs-chip--menu {
  border: 1px solid var(--rs-edgehi);
  color: var(--rs-faint);
  padding: 1px 5px;
}
</style>
