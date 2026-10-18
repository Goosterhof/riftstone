<script setup lang="ts">
// The stone's short sentences: bottom of the window, over the Rift, one
// polite live region. Undo lives here for 8 s.
import { useToasts } from "@/composables/useToasts";

const { toasts, dismiss, act } = useToasts();
</script>

<template>
  <div class="shelf" aria-live="polite" role="status">
    <div v-for="t in toasts" :key="t.id" class="toast" :class="`is-${t.tone}`">
      <p class="toast__line">
        <span v-if="t.inscription" class="toast__ins rs-inscription">{{ t.inscription }}</span
        >{{ t.line }}
      </p>
      <div class="toast__acts">
        <button v-for="a in t.actions" :key="a.label" type="button" class="toast__act" @click="act(t, a)">
          {{ a.label }}
        </button>
        <button type="button" class="toast__x" aria-label="Dismiss" @click="dismiss(t.id)">×</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.shelf {
  position: fixed;
  left: calc(var(--rs-pillar-w) + var(--rs-space-6));
  bottom: var(--rs-space-6);
  z-index: var(--rs-z-toast);
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-2);
  width: min(520px, calc(100vw - var(--rs-pillar-w) - 48px));
  pointer-events: none;
}
@media (max-width: 1199px) {
  .shelf {
    left: calc(var(--rs-pillar-w-narrow) + var(--rs-space-6));
  }
}
.toast {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--rs-space-3);
  padding: 10px var(--rs-space-3) 10px var(--rs-space-5);
  background: var(--rs-misthi);
  border: 1px solid var(--rs-edgehi);
  box-shadow: 6px 6px 0 rgba(0, 0, 0, 0.55);
  pointer-events: auto;
}
.toast::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 4px;
  background: var(--rs-rift);
}
.toast.is-world {
  background: var(--rs-hearthhi);
}
.toast.is-world::before {
  background: var(--rs-world);
}
.toast.is-ember::before {
  background: var(--rs-ember);
}
.toast.is-warn::before {
  background: var(--rs-warn);
}
.toast__line {
  flex: 1;
  margin: 0;
  line-height: 1.4;
}
.toast__acts {
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--rs-space-2);
}
.toast__act {
  height: 28px;
  padding: 0 10px;
  border: 1px solid var(--rs-edgehi);
  background: none;
  color: var(--rs-text);
  font: 500 var(--rs-text-sm) var(--rs-font-sans);
  cursor: pointer;
}
.toast__act:hover {
  background: var(--rs-mist);
}
.toast__x {
  width: 28px;
  height: 28px;
  border: 0;
  background: none;
  color: var(--rs-faint);
  font-size: 18px;
  cursor: pointer;
}
.toast__ins {
  display: block;
  margin-bottom: 2px;
  color: var(--rs-muted);
  font-size: var(--rs-text-xs);
  letter-spacing: 0.12em;
}
</style>
