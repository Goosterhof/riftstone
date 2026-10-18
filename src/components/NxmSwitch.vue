<script setup lang="ts">
// R16.24 — the nxm switch, the ACCOUNT card's last row. One switch whose
// accessible name never changes; its state a plain mono word at the right, no
// chip (ON in
// the Rift's cyan, OFF muted: off is a legitimate choice, never a warning);
// under it one line saying who answers Nexus's Mod Manager Download button now
// and who gets it back. The stone never turns it on by itself.
import { computed, onMounted } from "vue";
import { useNxmHandler } from "@/composables/useNxmHandler";

const nxm = useNxmHandler();

const state = computed(() => nxm.handler.value?.state ?? null);
const on = computed(() => state.value === "ours");
const disabled = computed(() => state.value === null || state.value === "unsupported" || nxm.busy.value);

const why = computed(() => {
  const h = nxm.handler.value;
  if (!h) return nxm.failure.value ? nxm.failure.value.voice : "Asking Windows who answers it…";
  if (h.state === "ours")
    return `The stone answers it now. Turning this off gives it back to ${h.givesBackTo ?? "no program"}.`;
  if (h.state === "none") return "No program answers it now.";
  if (h.state === "other")
    return `${h.other ?? "Another program"} answers it now. Turn this on and the stone remembers it, and gives it back when you turn this off.`;
  return "Only on Windows.";
});

function flip(): void {
  if (disabled.value) return;
  void nxm.set(!on.value);
}

onMounted(() => {
  if (!nxm.handler.value) void nxm.load();
});
</script>

<template>
  <div class="nxm">
    <button
      type="button"
      role="switch"
      class="nxm__switch"
      :aria-checked="on"
      :aria-disabled="disabled || undefined"
      :aria-busy="nxm.busy.value || undefined"
      aria-describedby="nxm-why"
      @click="flip"
    >
      <span class="nxm__label">Answer Nexus's Mod Manager Download button</span>
      <span class="nxm__state rs-mono" :class="{ 'is-on': on }" aria-hidden="true">{{ on ? "ON" : "OFF" }}</span>
    </button>
    <p id="nxm-why" class="nxm__why" :data-nxm-state="state ?? 'unknown'">{{ why }}</p>
  </div>
</template>

<style scoped>
.nxm {
  margin-top: var(--rs-space-3);
  padding-top: var(--rs-space-3);
  border-top: 1px solid var(--rs-edge);
}
.nxm__switch {
  display: flex;
  width: 100%;
  min-height: 32px;
  align-items: center;
  justify-content: space-between;
  gap: var(--rs-space-3);
  padding: 0;
  border: 0;
  background: none;
  color: var(--rs-text);
  font: 500 var(--rs-text-sm) var(--rs-font-sans);
  text-align: left;
  cursor: pointer;
}
.nxm__switch[aria-disabled="true"] {
  cursor: not-allowed;
}
.nxm__switch[aria-disabled="true"] .nxm__label {
  color: var(--rs-muted);
}
.nxm__switch:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: 2px;
}
/* a plain mono word, never a chip: no border, padding or radius */
.nxm__state {
  min-width: 3ch;
  color: var(--rs-muted);
  font-size: var(--rs-text-xs);
  font-weight: 600;
  letter-spacing: 0.04em;
  text-align: right;
}
.nxm__state.is-on {
  color: var(--rs-rift);
}
.nxm__why {
  margin: 2px 0 0;
  color: var(--rs-faint);
  font-size: var(--rs-text-sm);
}
</style>
