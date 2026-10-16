<script setup lang="ts">
// A screenshot window: 4:3, object-fit cover, lazy. No picture → a carved
// blank that says so (never a broken image).
import { computed, toRef } from "vue";
import type { ThumbnailSize } from "@/types/rift";
import { useThumbnail } from "@/composables/useThumbnails";

const {
  optionId,
  hasScreenshot,
  size = "card",
  alt = "",
} = defineProps<{ optionId: string | null; hasScreenshot: boolean; size?: ThumbnailSize; alt?: string }>();

const wanted = computed(() => hasScreenshot);
const src = useThumbnail(
  toRef(() => optionId),
  size,
  wanted,
);
</script>

<template>
  <div class="rs-thumb" :class="{ 'has-image': hasScreenshot && src }">
    <img v-if="src" :src="src" :alt="alt" loading="lazy" decoding="async" />
    <span v-else-if="!hasScreenshot" class="rs-thumb__blank" aria-hidden="true">NO PICTURE</span>
  </div>
</template>

<style scoped>
.rs-thumb {
  position: relative;
  overflow: hidden;
  background:
    repeating-linear-gradient(135deg, transparent 0 6px, rgba(255, 255, 255, 0.018) 6px 7px),
    linear-gradient(160deg, #161d27, #0d1218);
}
.rs-thumb img {
  display: block;
  width: 100%;
  height: 100%;
  max-width: none;
  object-fit: cover;
  transition:
    filter var(--rs-dur-quick),
    transform var(--rs-dur-quick);
}
.rs-thumb__blank {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  color: var(--rs-faint);
  font: 600 var(--rs-text-xs) var(--rs-font-mono);
  letter-spacing: 0.14em;
}
</style>
