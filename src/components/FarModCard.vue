<script setup lang="ts">
// A Nexus mod as the far world shows it (R16.19, R16.20): the v0.2 Find on
// Nexus candidate card, unchanged in its parts (name, ADULT, the quiet line,
// the summary), drawn at once (§11.1: results never stagger in). The name
// opens the mod's files; "In your mods folder" is the Rift's cyan outline
// (a package in the library, not an install).
import type { NexusCandidate } from "@/types/rift";
import { candidateLine } from "@/composables/useFarWorld";

const { mod } = defineProps<{ mod: NexusCandidate }>();
const emit = defineEmits<{ files: [mod: NexusCandidate] }>();
</script>

<template>
  <li class="fcard" :data-mod="mod.modId">
    <div class="fcard__body">
      <div class="fcard__line">
        <button
          type="button"
          class="fcard__name"
          :title="`Show the files of ${mod.name} on Nexus`"
          @click="emit('files', mod)"
        >
          {{ mod.name }}
        </button>
        <span v-if="mod.adult" class="fcard__mark rs-mono" title="Nexus marks this mod as adult content">ADULT</span>
        <span v-if="mod.linkedPackages > 0" class="fcard__have rs-mono">In your mods folder</span>
      </div>
      <p v-if="candidateLine(mod)" class="fcard__meta">{{ candidateLine(mod) }}</p>
      <p v-if="mod.summary" class="fcard__sum" :title="mod.summary">{{ mod.summary }}</p>
    </div>
    <div class="fcard__acts"><slot /></div>
  </li>
</template>

<style scoped>
.fcard {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--rs-space-3);
  padding: 8px var(--rs-space-3);
  background: var(--rs-mist);
  margin-bottom: 2px;
}
.fcard__body {
  min-width: 0;
}
.fcard__line {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--rs-space-2);
}
.fcard__name {
  padding: 0;
  border: 0;
  background: none;
  color: var(--rs-text);
  font: 700 var(--rs-text-md) var(--rs-font-sans);
  line-height: 1.25;
  text-align: left;
  overflow-wrap: anywhere;
  cursor: pointer;
}
.fcard__name:hover {
  text-decoration: underline;
  text-decoration-color: var(--rs-edgehi);
  text-underline-offset: 3px;
}
.fcard__name:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: 2px;
}
.fcard__mark,
.fcard__have {
  padding: 2px 6px;
  border: 1px solid var(--rs-edgehi);
  border-radius: var(--rs-radius-chip);
  color: var(--rs-muted);
  font-size: var(--rs-text-xs);
  font-weight: 600;
  letter-spacing: 0.04em;
}
.fcard__have {
  border-color: var(--rs-rift);
  color: var(--rs-rift);
}
.fcard__meta,
.fcard__sum {
  margin: 2px 0 0;
  font-size: var(--rs-text-sm);
}
.fcard__meta {
  color: var(--rs-faint);
}
.fcard__sum {
  overflow: hidden;
  color: var(--rs-muted);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.fcard__acts {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: var(--rs-space-2);
}
@media (max-width: 720px) {
  .fcard {
    grid-template-columns: minmax(0, 1fr);
  }
  .fcard__acts {
    justify-content: flex-start;
  }
}
</style>
