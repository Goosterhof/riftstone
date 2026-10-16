<script setup lang="ts">
// §8.3 lookbook — compare variants by their screenshots. 4:3 tiles, 176 wide
// (159 at the narrow window), a section per menu node, the state rail along
// each tile's foot. Arrows move in 2D; Enter (or a click) unfolds Your World
// with that variant; Space opens the lightbox, where ←/→ flip through every
// tile so two capes can be compared at full size. Hover or focus a tile and
// its description surfaces over the picture (the variant names alone rarely
// say which cape is which).
import { computed, nextTick, ref } from "vue";
import { usePackage, type LookbookTile } from "@/composables/usePackage";
import { useNavigation } from "@/composables/useNavigation";
import { useSettings } from "@/composables/useSettings";
import { useWorld } from "@/composables/useWorld";
import { rift } from "@/services/rift";
import RiftThumb from "./RiftThumb.vue";

const emit = defineEmits<{ chosen: [optionId: string] }>();

const pkg = usePackage();
const nav = useNavigation();
const world = useWorld();
const { settings } = useSettings();

const grid = ref<HTMLElement | null>(null);
const focusIndex = ref(0);
const blurbs = ref<Record<string, string>>({});

const flat = computed(() => pkg.tiles.value);
const indexOf = (t: LookbookTile) => flat.value.findIndex((x) => x.optionId === t.optionId);

async function blurb(optionId: string): Promise<void> {
  if (optionId in blurbs.value) return;
  blurbs.value = { ...blurbs.value, [optionId]: "" };
  try {
    const backend = await rift();
    const detail = await backend.getOption(optionId);
    blurbs.value = { ...blurbs.value, [optionId]: detail.option.info?.description ?? "" };
  } catch {
    // The ghost is a garnish: a missing description leaves the tile as it was.
  }
}

function columns(): number {
  const el = grid.value;
  if (!el) return 1;
  const tracks = getComputedStyle(el).gridTemplateColumns.split(" ").filter(Boolean);
  return Math.max(1, tracks.length);
}

async function focusTile(index: number): Promise<void> {
  const clamped = Math.max(0, Math.min(flat.value.length - 1, index));
  focusIndex.value = clamped;
  await nextTick();
  grid.value?.querySelector<HTMLElement>(`[data-tile="${clamped}"]`)?.focus();
}

function choose(tile: LookbookTile): void {
  void world.open(tile.optionId);
  emit("chosen", tile.optionId);
}

function zoom(index: number): void {
  const tile = flat.value.at(index);
  if (tile?.hasScreenshot) nav.openLightbox({ optionId: tile.optionId, name: tile.name });
}

/** ↑/↓ go to the tile that is visually above or below — the nearest tile in
 *  the next row, by centre — so a section break (a short last row, a new
 *  heading) does not throw the keyboard into another column. Without layout
 *  (a test DOM), the flat index steps by the column count. */
function verticalTarget(index: number, dir: 1 | -1): number {
  const el = grid.value;
  const from = el?.querySelector<HTMLElement>(`[data-tile="${index}"]`);
  if (!el || !from) return index + dir * columns();
  const here = from.getBoundingClientRect();
  const boxes = [...el.querySelectorAll<HTMLElement>("[data-tile]")].map((t) => ({
    i: Number(t.dataset.tile),
    r: t.getBoundingClientRect(),
  }));
  const ahead = boxes.filter((b) => (dir > 0 ? b.r.top > here.bottom - 1 : b.r.bottom < here.top + 1));
  if (!ahead.length || here.height === 0) return here.height === 0 ? index + dir * columns() : index;
  const rowTop = dir > 0 ? Math.min(...ahead.map((b) => b.r.top)) : Math.max(...ahead.map((b) => b.r.top));
  const centre = here.left + here.width / 2;
  const row = ahead.filter((b) => Math.abs(b.r.top - rowTop) < 2);
  const best = row.reduce((a, b) =>
    Math.abs(b.r.left + b.r.width / 2 - centre) < Math.abs(a.r.left + a.r.width / 2 - centre) ? b : a,
  );
  return best.i;
}

const MOVES: Partial<Record<string, (index: number) => number>> = {
  ArrowRight: (i) => i + 1,
  ArrowLeft: (i) => i - 1,
  ArrowDown: (i) => verticalTarget(i, 1),
  ArrowUp: (i) => verticalTarget(i, -1),
};

function onKey(event: KeyboardEvent, index: number): void {
  const move = MOVES[event.key];
  if (move) {
    event.preventDefault();
    void focusTile(move(index));
    return;
  }
  const tile = flat.value.at(index);
  if (event.key === "Enter" && tile) choose(tile);
  else if (event.key === " ") {
    event.preventDefault();
    zoom(index);
  }
}

defineExpose({
  focusFirst: () =>
    focusTile(
      Math.max(
        0,
        flat.value.findIndex((t) => t.optionId === world.selectedId.value),
      ),
    ),
});
</script>

<template>
  <div
    v-if="pkg.detail.value"
    ref="grid"
    class="lookbook"
    :class="{ 'is-veiled-all': settings.veilThumbnails }"
    role="grid"
    aria-label="Lookbook"
  >
    <template v-for="s in pkg.sections.value" :key="s.heading">
      <h3 class="lookbook__head rs-inscription" role="row">{{ s.heading }}</h3>
      <button
        v-for="t in s.tiles"
        :key="t.optionId"
        type="button"
        class="tile"
        :class="{
          'is-installed': t.installed,
          'is-veiled': settings.veilThumbnails,
          'is-sel': world.selectedId.value === t.optionId,
        }"
        :data-tile="indexOf(t)"
        :tabindex="indexOf(t) === focusIndex ? 0 : -1"
        :aria-label="`${t.name}, ${t.installed ? 'installed' : 'not installed'}`"
        @click="choose(t)"
        @focus="
          focusIndex = indexOf(t);
          blurb(t.optionId);
        "
        @mouseenter="blurb(t.optionId)"
        @keydown="onKey($event, indexOf(t))"
      >
        <span class="tile__frame">
          <RiftThumb
            class="tile__thumb"
            :option-id="t.optionId"
            :has-screenshot="t.hasScreenshot"
            :alt="`Screenshot of ${t.name}`"
          />
          <span v-if="blurbs[t.optionId]" class="tile__ghost">{{ blurbs[t.optionId] }}</span>
        </span>
        <span class="tile__name">{{ t.short }}</span>
        <span class="tile__crumb">{{ t.installed ? "INSTALLED" : t.crumb }}</span>
      </button>
    </template>
  </div>
  <p v-else class="lookbook__wait rs-mono">Opening the package…</p>
</template>

<style scoped>
.lookbook {
  display: grid;
  grid-template-columns: repeat(auto-fill, 176px);
  gap: var(--rs-space-3);
  align-content: start;
  padding: var(--rs-space-4) var(--rs-space-6) var(--rs-space-8);
}
@media (max-width: 1199px) {
  .lookbook {
    grid-template-columns: repeat(auto-fill, 159px);
  }
}
.lookbook__head {
  grid-column: 1 / -1;
  margin: var(--rs-space-3) 0 0;
  padding: 6px 0;
  border-bottom: 1px solid var(--rs-edge);
  color: var(--rs-muted);
  font-size: 14px;
}
.lookbook__wait {
  padding: var(--rs-space-6);
  color: var(--rs-faint);
}
.tile {
  position: relative;
  display: flex;
  flex-direction: column;
  padding: 0 0 4px;
  border: 0;
  background: var(--rs-mist);
  color: var(--rs-text);
  text-align: left;
  cursor: pointer;
  transition: background-color var(--rs-dur-quick);
}
/* the state rail along the tile's foot: cyan, gold when it crossed */
.tile::after {
  content: "";
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 4px;
  background: linear-gradient(to right, var(--rs-world) 50%, var(--rs-rift) 50%);
  background-size: 200% 100%;
  background-position: 100% 0;
  transition: background-position var(--rs-dur-crossing) var(--rs-ease-carve);
}
.tile.is-installed::after {
  background-position: 0 0;
}
.tile:hover,
.tile.is-sel {
  background: var(--rs-misthi);
}
.tile.is-sel {
  outline: 2px solid var(--rs-rift);
  outline-offset: 2px;
}
.tile__frame {
  position: relative;
  display: block;
  aspect-ratio: 4 / 3;
}
.tile__thumb {
  width: 100%;
  height: 100%;
}
/* The Ghost: the description surfaces on hover/focus only. */
.tile__ghost {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  padding: 18px 8px 6px;
  background: linear-gradient(rgba(10, 14, 19, 0), rgba(10, 14, 19, 0.92) 40%);
  color: var(--rs-text);
  font-size: var(--rs-text-sm);
  line-height: 1.35;
  opacity: 0;
  transition: opacity var(--rs-dur-quick);
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.tile:hover .tile__ghost,
.tile:focus-visible .tile__ghost {
  opacity: 1;
}
.tile__name {
  padding: 6px 8px 2px;
  font: 500 var(--rs-text-sm) / 1.25 var(--rs-font-sans);
}
.tile__crumb {
  padding: 0 8px 8px;
  color: var(--rs-faint);
  font-size: var(--rs-text-sm);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tile.is-installed .tile__crumb {
  color: var(--rs-world);
  font: 600 var(--rs-text-xs) var(--rs-font-mono);
  letter-spacing: 0.04em;
}
</style>
