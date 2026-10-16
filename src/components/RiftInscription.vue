<script setup lang="ts">
// The stone speaks (§9): an inscription carved in Cinzel, one plain body
// line, and the recovery actions — always at least one.
const {
  inscription = null,
  body,
  tone = "rift",
  size = "lg",
} = defineProps<{
  inscription?: string | null;
  body: string;
  tone?: "rift" | "world" | "ember" | "warn";
  size?: "lg" | "md";
}>();
</script>

<template>
  <section class="rs-ins" :class="[`rs-ins--${tone}`, `rs-ins--${size}`]" :role="tone === 'ember' ? 'alert' : 'status'">
    <h2 v-if="inscription" class="rs-ins__carve rs-inscription">{{ inscription }}</h2>
    <p class="rs-ins__body">{{ body }}</p>
    <div v-if="$slots.default" class="rs-ins__actions"><slot /></div>
  </section>
</template>

<style scoped>
.rs-ins {
  padding: var(--rs-space-12) var(--rs-space-8) var(--rs-space-8);
  max-width: 560px;
}
.rs-ins--md {
  padding: var(--rs-space-4) 0;
}
.rs-ins__carve {
  margin: 0 0 var(--rs-space-3);
  font-size: var(--rs-text-2xl);
  line-height: 1.1;
  color: var(--rs-text);
  /* carved: a hairline of light on the lower lip, a shade on the upper */
  text-shadow:
    0 1px 0 rgba(255, 255, 255, 0.06),
    0 -1px 0 rgba(0, 0, 0, 0.6);
}
.rs-ins--md .rs-ins__carve {
  font-size: var(--rs-text-xl);
}
.rs-ins--ember .rs-ins__carve {
  color: var(--rs-ember);
}
.rs-ins--warn .rs-ins__carve {
  color: var(--rs-warn);
}
.rs-ins__body {
  margin: 0;
  max-width: 460px;
  color: var(--rs-muted);
  line-height: 1.5;
}
.rs-ins__actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--rs-space-2);
  margin-top: var(--rs-space-5);
}
</style>
