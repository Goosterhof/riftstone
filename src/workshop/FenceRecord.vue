<script setup lang="ts">
// R20.13 with the contract's FenceView — the probe record under the head. It
// pushes the hollow down, never covers it. The record is the stone's: this
// fold renders it and adds nothing (Pattern 013). The control is the first
// step and must have landed before any refusal below it counts.
import { computed } from "vue";
import type { FenceView } from "@/types/workshop";
import RiftButton from "@/components/RiftButton.vue";
import { CHIP_WORD, EXPECT_WORD, PAWN_NAME, PROBE_HELD_BACK, stamp } from "./words";

const {
  fence,
  probing = false,
  summoned = false,
} = defineProps<{
  fence: FenceView;
  probing?: boolean;
  /** A pawn is summoned: the probe needs the workshop to itself. */
  summoned?: boolean;
}>();

const emit = defineEmits<{ run: [] }>();

const ground = computed(() => {
  const g = fence.ground;
  if (!g) return null;
  const parts = [
    `proved ${stamp(fence.provedAt)}`,
    g.distro,
    `claude ${g.claudeVersion}`,
    g.kernel,
    g.digest.slice(0, 8),
  ];
  return parts.join(" · ");
});
const control = computed(() => fence.steps.at(0) ?? null);
const noRecord = computed(() => fence.chip === "unprobed" && !fence.ground && fence.steps.length === 0);
</script>

<template>
  <section id="ws-fence-record" class="fence" aria-label="The fence's probe record">
    <p class="fence__title rs-mono">
      <span class="fence__carved">THE FENCE</span>
      <span class="fence__now">{{ CHIP_WORD[fence.chip] }}</span>
      <template v-if="ground"> — {{ ground }}</template>
    </p>
    <p v-if="noRecord" class="fence__line">No probe record matches this host. The stone claims nothing.</p>
    <p v-if="fence.changed" class="fence__line fence__line--warn">{{ fence.changed }}</p>
    <p v-if="control && !control.ok" class="fence__line fence__line--ember">
      The control did not succeed, so the refusals below prove nothing.
    </p>
    <ol v-if="fence.steps.length" class="fence__steps rs-mono">
      <li
        v-for="(s, i) in fence.steps"
        :key="i"
        class="fence__step"
        :class="{ 'is-broken': !s.ok, 'is-control': i === 0 }"
      >
        <span class="fence__mark" :aria-label="s.ok ? 'held' : 'did not hold'">{{ s.ok ? "✓" : "✕" }}</span>
        <span class="fence__what">{{ s.step }}</span>
        <span class="fence__expect">{{ EXPECT_WORD[s.expected] }}</span>
        <span class="fence__seen">{{ s.observed }}</span>
      </li>
    </ol>
    <p class="fence__line">Each step is read outside the fence, never from the pawn's answer.</p>
    <p class="fence__line">What {{ PAWN_NAME }} reads, the REFramework log included, leaves this PC for Anthropic.</p>
    <div class="fence__foot">
      <RiftButton
        size="sm"
        :busy="probing"
        :refused="summoned"
        :reason="summoned ? PROBE_HELD_BACK : null"
        @click="emit('run')"
      >
        Run the probe
      </RiftButton>
      <span v-if="summoned" class="fence__held">{{ PROBE_HELD_BACK }}</span>
      <span v-else-if="probing" class="fence__held">The stone is probing the fence…</span>
    </div>
  </section>
</template>

<style scoped>
.fence {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-2);
  padding: var(--rs-space-4) var(--rs-space-5);
  background: var(--rs-basalt);
  border-top: 1px solid var(--rs-edge);
  border-bottom: 1px solid var(--rs-edge);
}
.fence__title {
  margin: 0;
  color: var(--rs-faint);
}
.fence__carved {
  color: var(--rs-muted);
  font-weight: 600;
  letter-spacing: 0.12em;
}
.fence__now {
  margin-left: var(--rs-space-2);
  color: var(--rs-text);
  font-weight: 600;
}
.fence__line {
  margin: 0;
  color: var(--rs-muted);
  font: 400 var(--rs-text-xs) / 1.4 var(--rs-font-mono);
}
.fence__line--warn {
  color: var(--rs-warn);
}
.fence__line--ember {
  color: var(--rs-ember);
}
.fence__steps {
  display: grid;
  grid-template-columns: 1.5ch minmax(0, 1.3fr) 13ch minmax(0, 1.2fr);
  column-gap: var(--rs-space-3);
  row-gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
}
.fence__step {
  display: contents;
  color: var(--rs-muted);
}
.fence__step > span {
  padding: 1px 0;
  overflow-wrap: anywhere;
}
.fence__mark {
  color: var(--rs-text);
}
.fence__what {
  color: var(--rs-text);
}
.fence__expect {
  color: var(--rs-faint);
}
/* the control: the first step, and the one every refusal stands on */
.fence__step.is-control > span {
  padding-bottom: var(--rs-space-1);
  border-bottom: 1px solid var(--rs-edge);
  margin-bottom: 2px;
}
.fence__step.is-broken > span {
  color: var(--rs-ember);
}
.fence__foot {
  display: flex;
  align-items: center;
  gap: var(--rs-space-3);
  margin-top: var(--rs-space-1);
}
.fence__held {
  color: var(--rs-faint);
  font: 400 var(--rs-text-sm) var(--rs-font-sans);
}
</style>
