<script setup lang="ts">
// §7.7 — four variants, radius 0. Loading swaps the label for a mono "···"
// at the same width (aria-busy); disabled still says why in its title.
// Busy is aria-disabled, not disabled: a keyboard press must not drop the
// focus to <body> while the stone works (§6.3, the ring is never removed),
// so a busy button keeps its focus and swallows further presses.
// Refused is the same for a press the stone holds back (R16.21's keyless
// Download, as Play refused, R16.13): aria-disabled, still in the tab order,
// the click swallowed, and its reason linked by aria-describedby, so a
// keyboard reaches the button and hears why. Disabled takes it out of reach.
import { useId } from "vue";

const {
  variant = "secondary",
  size = "md",
  busy = false,
  disabled = false,
  refused = false,
  reason = null,
  type = "button",
} = defineProps<{
  variant?: "primary" | "world" | "secondary" | "danger";
  size?: "md" | "sm";
  busy?: boolean;
  disabled?: boolean;
  /** Held back but reachable: aria-disabled, focusable, `reason` its description. */
  refused?: boolean;
  reason?: string | null;
  type?: "button" | "submit";
}>();

const emit = defineEmits<{ click: [event: MouseEvent] }>();

const reasonId = useId();

function press(event: MouseEvent): void {
  if (busy || refused) {
    event.preventDefault();
    return;
  }
  emit("click", event);
}
</script>

<template>
  <button
    :type="type"
    class="rs-btn"
    :class="[`rs-btn--${variant}`, `rs-btn--${size}`, { 'is-busy': busy, 'is-refused': refused && !busy }]"
    :disabled="disabled"
    :aria-disabled="(busy || refused) && !disabled ? 'true' : undefined"
    :aria-busy="busy ? 'true' : undefined"
    :aria-describedby="refused && reason ? reasonId : undefined"
    :title="(disabled || refused) && reason ? reason : undefined"
    @click="press"
  >
    <span class="rs-btn__label"><slot /></span>
    <span v-if="busy" class="rs-btn__busy" aria-hidden="true">···</span>
    <!-- the reason, out of the name (hidden) and in the description (referenced) -->
    <span v-if="refused && reason" :id="reasonId" hidden>{{ reason }}</span>
  </button>
</template>

<style scoped>
.rs-btn {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--rs-space-2);
  height: 36px;
  padding: 0 var(--rs-space-4);
  border: 1px solid var(--rs-edgehi);
  border-radius: 0;
  background: transparent;
  color: var(--rs-text);
  font: 500 var(--rs-text-sm) / 1 var(--rs-font-sans);
  white-space: nowrap;
  cursor: pointer;
  transition:
    background-color var(--rs-dur-quick) var(--rs-ease-carve),
    transform var(--rs-dur-quick) var(--rs-ease-carve);
}
.rs-btn--sm {
  height: 28px;
  padding: 0 var(--rs-space-3);
}
.rs-btn:hover:not(:disabled, .is-busy, .is-refused) {
  background: var(--rs-misthi);
}
.rs-btn:active:not(:disabled, .is-busy, .is-refused) {
  transform: translateY(1px);
}
.rs-btn:disabled,
.rs-btn.is-refused {
  opacity: 0.45;
  cursor: not-allowed;
}
.rs-btn--primary {
  background: var(--rs-riftdeep);
  border-color: var(--rs-rift);
}
.rs-btn--primary:hover:not(:disabled, .is-busy, .is-refused) {
  background: #256673;
}
.rs-btn--world {
  background: var(--rs-world);
  border-color: var(--rs-world);
  color: var(--rs-ongold);
  font-weight: 700;
}
.rs-btn--world:hover:not(:disabled, .is-busy, .is-refused) {
  background: #f1d088;
}
.rs-btn--danger {
  border-color: var(--rs-ember);
  color: var(--rs-ember);
}
.rs-btn--danger:hover:not(:disabled, .is-busy, .is-refused) {
  background: rgba(255, 123, 102, 0.1);
}
/* Busy: keep the width, hide the label, show the 3-dot in the same colour. */
.is-busy .rs-btn__label {
  visibility: hidden;
}
.is-busy,
.is-busy:disabled {
  opacity: 1;
  cursor: progress;
}
.rs-btn__busy {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  font: 600 14px var(--rs-font-mono);
  letter-spacing: 0.2em;
}
</style>
