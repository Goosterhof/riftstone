<script setup lang="ts">
// The full-window screenshot. Opens unveiled (an explicit act, §8.6). In the
// lookbook ←/→ step through every variant so two can be compared full size.
import { computed, nextTick, ref, watch } from "vue";
import { useNavigation } from "@/composables/useNavigation";
import { usePackage } from "@/composables/usePackage";
import RiftThumb from "./RiftThumb.vue";

const nav = useNavigation();
const pkg = usePackage();
const closer = ref<HTMLButtonElement | null>(null);

const ring = computed(() => (nav.riftMode.value === "lookbook" ? pkg.tiles.value.filter((t) => t.hasScreenshot) : []));
const at = computed(() => ring.value.findIndex((t) => t.optionId === nav.lightbox.value?.optionId));

function step(delta: number): void {
  if (at.value < 0 || !ring.value.length) return;
  const next = ring.value.at((at.value + delta + ring.value.length) % ring.value.length);
  if (next) nav.openLightbox({ optionId: next.optionId, name: next.name });
}

function onKey(event: KeyboardEvent): void {
  if (event.key === "Escape" || event.key === " ") {
    event.preventDefault();
    nav.closeLightbox();
  } else if (event.key === "ArrowRight") step(1);
  else if (event.key === "ArrowLeft") step(-1);
}

/** Where the keyboard stood when the lightbox opened. */
let opener: HTMLElement | null = null;

/** Closing hands focus back: to the card or tile of the picture last shown
 *  (←/→ may have stepped to another variant), else to whatever opened it. */
function restoreFocus(optionId: string): void {
  const tile = pkg.tiles.value.findIndex((t) => t.optionId === optionId);
  const target =
    (tile >= 0 ? document.querySelector<HTMLElement>(`[data-tile="${tile}"]`) : null) ??
    document.querySelector<HTMLElement>(`[data-option="${CSS.escape(optionId)}"]`) ??
    opener;
  if (target?.isConnected) target.focus();
  opener = null;
}

watch(
  () => nav.lightbox.value,
  async (open, was) => {
    if (open && !was) {
      const active = document.activeElement;
      opener = active instanceof HTMLElement && active !== document.body ? active : null;
    }
    await nextTick();
    if (open) closer.value?.focus();
    else if (was) restoreFocus(was.optionId);
  },
);
</script>

<template>
  <div
    v-if="nav.lightbox.value"
    class="lightbox"
    role="dialog"
    aria-modal="true"
    :aria-label="`Screenshot of ${nav.lightbox.value.name}`"
    @keydown="onKey"
    @click.self="nav.closeLightbox()"
  >
    <figure class="lightbox__fig">
      <RiftThumb
        class="lightbox__img"
        :option-id="nav.lightbox.value.optionId"
        :has-screenshot="true"
        size="full"
        :alt="`Screenshot of ${nav.lightbox.value.name}`"
      />
      <figcaption class="lightbox__cap">
        <span>{{ nav.lightbox.value.name }}</span>
        <span v-if="ring.length > 1" class="rs-mono">{{ at + 1 }} / {{ ring.length }} · ← → to compare</span>
        <button ref="closer" type="button" class="lightbox__x" @click="nav.closeLightbox()">Close · Esc</button>
      </figcaption>
    </figure>
  </div>
</template>

<style scoped>
.lightbox {
  position: fixed;
  inset: 0;
  z-index: 60;
  display: grid;
  place-items: center;
  padding: var(--rs-space-8);
  background: rgba(5, 7, 10, 0.92);
}
.lightbox__fig {
  margin: 0;
  width: min(100%, calc((100vh - 120px) * 4 / 3));
}
.lightbox__img {
  width: 100%;
  aspect-ratio: 4 / 3;
}
.lightbox__img :deep(img) {
  object-fit: contain;
  background: #000;
}
.lightbox__cap {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--rs-space-4);
  margin-top: var(--rs-space-3);
  color: var(--rs-muted);
}
.lightbox__x {
  height: 32px;
  padding: 0 12px;
  border: 1px solid var(--rs-edgehi);
  background: none;
  color: var(--rs-text);
  font: inherit;
  cursor: pointer;
}
</style>
