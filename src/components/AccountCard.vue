<script setup lang="ts">
// R16.17 — the ACCOUNT card: what the key is and what it does for this
// account. It names the account, never the key (not masked, not its last
// four) and never the e-mail: the key stays in its masked row. The tier is a
// neutral outlined badge (colour is state, and a tier is not a state of your
// world); the one sentence under it says what this account's key unlocks.
import { computed } from "vue";
import { useFarWorld } from "@/composables/useFarWorld";
import { useNexusAccount } from "@/composables/useNexusAccount";
import { formatClock } from "@/services/format";
import FailureNotice from "./FailureNotice.vue";
import NxmSwitch from "./NxmSwitch.vue";
import RiftButton from "./RiftButton.vue";

const emit = defineEmits<{ replaceKey: []; removeKey: [] }>();

const nexus = useNexusAccount();
const far = useFarWorld();

const PREMIUM_LINE =
  "Your premium account lets the stone download mods straight into your mods folder, and track and endorse them on Nexus.";
const FREE_LINE =
  "With a free account Nexus sends downloads through its own Mod Manager Download button; the stone tracks and endorses mods for you here.";

const quotaLine = computed(() => {
  const a = nexus.account.value;
  const q = far.quota.value;
  const parts: string[] = [];
  if (a) parts.push(`Checked ${formatClock(a.checkedMs)}`);
  if (q) {
    parts.push(`hourly ${q.hourlyRemaining}/${q.hourlyLimit}`);
    parts.push(`daily ${q.dailyRemaining.toLocaleString("en-GB")}/${q.dailyLimit.toLocaleString("en-GB")}`);
  }
  return parts.join(" · ");
});
</script>

<template>
  <article v-if="nexus.account.value" class="acct" aria-labelledby="acct-name">
    <p class="acct__who">
      <span id="acct-name" class="acct__name">{{ nexus.account.value.name }}</span>
      <span class="acct__tier rs-mono">{{ nexus.premium.value ? "PREMIUM" : "FREE" }}</span>
      <span v-if="nexus.account.value.supporter" class="acct__supporter">supporter</span>
    </p>
    <p class="acct__quota rs-mono">{{ quotaLine }}</p>
    <p class="acct__unlock" data-unlock>{{ nexus.premium.value ? PREMIUM_LINE : FREE_LINE }}</p>
    <FailureNotice
      v-if="nexus.accountFailure.value"
      :voiced="nexus.accountFailure.value"
      :busy="nexus.refreshing.value"
      @recover="(kind) => (kind === 'replaceKey' ? emit('replaceKey') : void nexus.refreshAccount())"
    />
    <div class="acct__acts">
      <RiftButton size="sm" :busy="nexus.refreshing.value" @click="nexus.refreshAccount()">Check again</RiftButton>
      <RiftButton size="sm" @click="emit('replaceKey')">Replace key…</RiftButton>
      <RiftButton size="sm" variant="danger" @click="emit('removeKey')">Remove key</RiftButton>
    </div>
    <NxmSwitch />
  </article>
</template>

<style scoped>
.acct {
  padding: var(--rs-space-3) var(--rs-space-4);
  background: var(--rs-mist);
  border: 1px solid var(--rs-edge);
}
.acct__who {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--rs-space-3);
  margin: 0;
}
.acct__name {
  color: var(--rs-text);
  font: 700 var(--rs-text-md) var(--rs-font-sans);
  overflow-wrap: anywhere;
}
.acct__tier {
  padding: 2px 6px;
  border: 1px solid var(--rs-edgehi);
  border-radius: var(--rs-radius-chip);
  color: var(--rs-muted);
  font-size: var(--rs-text-xs);
  font-weight: 600;
  letter-spacing: 0.04em;
}
.acct__supporter {
  color: var(--rs-faint);
  font-size: var(--rs-text-sm);
}
.acct__quota {
  margin: 4px 0 0;
  color: var(--rs-faint);
  font-size: var(--rs-text-xs);
}
.acct__unlock {
  margin: var(--rs-space-2) 0 0;
  color: var(--rs-text);
  font-size: var(--rs-text-sm);
  max-width: 64ch;
}
.acct__acts {
  display: flex;
  flex-wrap: wrap;
  gap: var(--rs-space-2);
  margin-top: var(--rs-space-3);
}
</style>
