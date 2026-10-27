<script setup lang="ts">
// A vocation's mark: drawn by the lab, straight cuts and at most one ring
// (RF.10). Keyed by the vocation's name; anything else draws the plain bar.
import { computed } from "vue";

const { vocation } = defineProps<{ vocation: string | null }>();

type Shape = { tag: "rect" | "line" | "polyline" | "circle"; attrs: Record<string, string> };
const line = (x1: number, y1: number, x2: number, y2: number): Shape => ({
  tag: "line",
  attrs: { x1: String(x1), y1: String(y1), x2: String(x2), y2: String(y2) },
});
const MARKS: Record<string, Shape[]> = {
  Fighter: [{ tag: "rect", attrs: { x: "9", y: "6", width: "30", height: "36" } }, line(24, 12, 24, 36)],
  Warrior: [line(10, 38, 40, 8), line(11, 25, 23, 37)],
  Archer: [line(5, 24, 42, 24), { tag: "polyline", attrs: { points: "32,14 42,24 32,34" } }],
  Thief: [line(13, 13, 35, 35), line(35, 13, 13, 35)],
  Mage: [
    { tag: "circle", attrs: { cx: "24", cy: "24", r: "15" } },
    { tag: "rect", attrs: { x: "22", y: "22", width: "4", height: "4" } },
  ],
  Sorcerer: [
    { tag: "circle", attrs: { cx: "24", cy: "31", r: "11" } },
    line(24, 5, 24, 13),
    line(11, 9, 15, 16),
    line(37, 9, 33, 16),
  ],
};
const OTHER: Shape[] = [line(10, 24, 38, 24)];
const shapes = computed(() => (vocation ? (MARKS[vocation] ?? OTHER) : OTHER));
</script>

<template>
  <svg
    viewBox="0 0 48 48"
    aria-hidden="true"
    fill="none"
    stroke="currentColor"
    stroke-width="4"
    stroke-linecap="square"
    stroke-linejoin="miter"
  >
    <component :is="s.tag" v-for="(s, i) in shapes" :key="i" v-bind="s.attrs" />
  </svg>
</template>
