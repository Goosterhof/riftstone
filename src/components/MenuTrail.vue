<script setup lang="ts">
// §7.4 — the menu trail a variant hides behind. Each crumb is a button that
// opens the package view at that node. Long trails fold their middle to "…"
// with the whole trail in the title. "(Menu Option)" stays verbatim.
import { computed } from "vue";
import HitText from "./HitText.vue";

const {
  trail,
  tokens = [],
  interactive = true,
  tabbable = true,
} = defineProps<{ trail: string[]; tokens?: string[]; interactive?: boolean; tabbable?: boolean }>();

const emit = defineEmits<{ crumb: [name: string, index: number] }>();

interface Crumb {
  name: string;
  index: number;
  folded: boolean;
  /** The menu crumb and the "(Menu Option)" crumb hold their width. */
  holds: boolean;
}

const FOLD_AT = 64;

const crumbs = computed<Crumb[]>(() => {
  // Shrink order (PathProof's rule): the collection crumb gives way first, the
  // middle next; the last two crumbs — where a variant hides, and the
  // "(Menu Option)" crumb — never shrink.
  const all = trail.map((name, index) => ({ name, index, folded: false, holds: index >= trail.length - 2 }));
  const long = trail.join(" › ").length > FOLD_AT;
  if (!long || trail.length < 4) return all;
  const first = all.slice(0, 1);
  const last = all.slice(-2);
  return [...first, { name: "…", index: -1, folded: true, holds: true }, ...last];
});

const whole = computed(() => trail.join(" › "));
</script>

<template>
  <nav class="rs-trail" :aria-label="`Menu trail: ${whole}`" :title="whole">
    <template v-for="(c, i) in crumbs" :key="`${c.index}-${c.name}`">
      <span v-if="i > 0" class="rs-trail__sep" aria-hidden="true">›</span>
      <span v-if="c.folded" class="rs-trail__fold">…</span>
      <button
        v-else-if="interactive"
        type="button"
        class="rs-trail__crumb"
        :class="{ 'is-first': c.index === 0 && trail.length > 1, 'is-held': c.holds && c.index > 0 }"
        :tabindex="tabbable ? undefined : -1"
        @click.stop="emit('crumb', c.name, c.index)"
      >
        <HitText :text="c.name" :tokens="tokens" />
      </button>
      <span
        v-else
        class="rs-trail__crumb is-static"
        :class="{ 'is-first': c.index === 0 && trail.length > 1, 'is-held': c.holds && c.index > 0 }"
        ><HitText :text="c.name" :tokens="tokens"
      /></span>
    </template>
  </nav>
</template>

<style scoped>
.rs-trail {
  display: flex;
  align-items: baseline;
  min-width: 0;
  overflow: hidden;
  color: var(--rs-muted);
  font: 400 var(--rs-text-sm) / 1.45 var(--rs-font-sans);
  white-space: nowrap;
}
.rs-trail__sep {
  flex: none;
  margin: 0 5px;
  color: var(--rs-faint);
}
.rs-trail__fold {
  flex: none;
  color: var(--rs-faint);
}
.rs-trail__crumb {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  padding: 0;
  border: 0;
  background: none;
  color: inherit;
  font: inherit;
  cursor: pointer;
  flex: 0 1 auto;
}
/* the collection crumb gives way first, down to a stub */
.rs-trail__crumb.is-first {
  flex-shrink: 1000;
  min-width: 2ch;
}
/* the menu crumb and "(Menu Option)": where the variant hides, never cut
   unless the whole trail alone is wider than the card */
.rs-trail__crumb.is-held {
  flex-shrink: 0.001;
}
.rs-trail__crumb:hover:not(.is-static) {
  color: var(--rs-text);
  text-decoration: underline;
  text-underline-offset: 3px;
}
.rs-trail__crumb.is-static {
  cursor: inherit;
}
</style>
