<script setup lang="ts">
// §7.2 — the search well carved into the stone. Esc clears; a second Esc
// blurs. ↓ steps down into the answers; Enter opens the first (or active) one.
import { ref } from "vue";

const { modelValue } = defineProps<{ modelValue: string }>();
const emit = defineEmits<{
  "update:modelValue": [value: string];
  clear: [];
  descend: [];
  submit: [];
}>();

const input = ref<HTMLInputElement | null>(null);

function onEscape(): void {
  if (modelValue) emit("clear");
  else input.value?.blur();
}

function onInput(event: Event): void {
  emit("update:modelValue", (event.target as HTMLInputElement).value);
}

defineExpose({ focus: () => input.value?.focus(), el: input });
</script>

<template>
  <label class="rs-well">
    <span class="rs-sr-only">Search</span>
    <svg class="rs-well__glyph" viewBox="0 0 16 16" aria-hidden="true">
      <circle cx="6.5" cy="6.5" r="4.5" fill="none" stroke="currentColor" stroke-width="1.6" />
      <path d="M10 10l4 4" stroke="currentColor" stroke-width="1.6" />
    </svg>
    <input
      id="rift-search"
      ref="input"
      :value="modelValue"
      type="search"
      autocomplete="off"
      spellcheck="false"
      placeholder="Search names, menus, file paths…"
      aria-controls="rift-answers"
      @input="onInput"
      @keydown.esc.prevent="onEscape"
      @keydown.down.prevent="emit('descend')"
      @keydown.enter.prevent="emit('submit')"
    />
    <kbd class="rs-well__key" title="Press / or Ctrl+K to search">/</kbd>
  </label>
</template>

<style scoped>
.rs-well {
  display: flex;
  align-items: center;
  gap: var(--rs-space-2);
  height: 44px;
  padding: 0 10px;
  background: var(--rs-well);
  border: 1px solid var(--rs-edgehi);
  /* carved: the well sits below the stone's face */
  box-shadow: inset 0 2px 0 rgba(0, 0, 0, 0.45);
  transition:
    box-shadow var(--rs-dur-quick),
    border-color var(--rs-dur-quick);
}
.rs-well:focus-within {
  border-color: var(--rs-rift);
  box-shadow:
    inset 0 2px 0 rgba(0, 0, 0, 0.45),
    var(--rs-glow-rift);
}
.rs-well__glyph {
  flex: none;
  width: 15px;
  height: 15px;
  color: var(--rs-rift);
}
.rs-well input {
  flex: 1;
  min-width: 0;
  height: 100%;
  border: 0;
  background: none;
  color: var(--rs-text);
  font: 400 var(--rs-text-sm) var(--rs-font-sans);
}
/* the well's own focus is its glow; the ring would double it */
.rs-well input:focus-visible {
  outline: none;
}
.rs-well input::placeholder {
  color: var(--rs-faint);
}
.rs-well input::-webkit-search-cancel-button {
  display: none;
}
.rs-well__key {
  flex: none;
  padding: 1px 5px;
  border: 1px solid var(--rs-edgehi);
  border-radius: var(--rs-radius-chip);
  color: var(--rs-muted);
  font: 600 var(--rs-text-xs) var(--rs-font-mono);
}
</style>
