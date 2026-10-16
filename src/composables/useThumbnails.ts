// Screenshots, fetched through get_thumbnail once per option and size, cached
// for the session. A card asks; the cache answers or fetches.

import { ref, watch, type Ref } from "vue";
import type { ThumbnailSize } from "@/types/rift";
import { rift } from "@/services/rift";

const cache = new Map<string, Promise<string | null>>();

export function fetchThumbnail(optionId: string, size: ThumbnailSize): Promise<string | null> {
  const key = `${size}|${optionId}`;
  let pending = cache.get(key);
  if (!pending) {
    pending = rift()
      .then((backend) => backend.getThumbnail(optionId, size))
      .catch(() => null);
    cache.set(key, pending);
  }
  return pending;
}

/** A reactive src for an option's screenshot; null until (or unless) it exists. */
export function useThumbnail(optionId: Ref<string | null>, size: ThumbnailSize, wanted: Ref<boolean>) {
  const src = ref<string | null>(null);
  watch(
    [optionId, wanted],
    async ([id, want]) => {
      src.value = null;
      if (!id || !want) return;
      const url = await fetchThumbnail(id, size);
      if (optionId.value === id) src.value = url;
    },
    { immediate: true },
  );
  return src;
}

export function resetThumbnails(): void {
  cache.clear();
}
