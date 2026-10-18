<script setup lang="ts">
// §8.2 — the selected option in Your World: screenshot (click → lightbox),
// name, trail, by-line, description, the three doors (Install…, Open in
// package, Show in archive), conflicts, payload and skipped files. Install…
// opens THE CROSSING in place; the screenshot shrinks to a strip for it.
// v0.2: the NEXUS line — the mod's category and tags as the pillar's quiet
// chips (pressing one toggles that facet), "Nexus #id" as the door to the
// mod's page, and an honest line when the stone has no labels for it.
// v0.5 (#00015 R18.4): an option of a family says, under its kinds, how it
// stands against the file's other versions, and carries the door that needs:
// `Install v1.3…` (THE CROSSING carries the replace) or `Keep only the newest…`.
import { computed } from "vue";
import type { OptionDetail } from "@/types/rift";
import { useWorld } from "@/composables/useWorld";
import { useNavigation } from "@/composables/useNavigation";
import { useSearch } from "@/composables/useSearch";
import { usePreflight } from "@/composables/usePreflight";
import { useSettings } from "@/composables/useSettings";
import { useDownloads } from "@/composables/useDownloads";
import { useNexusAccount } from "@/composables/useNexusAccount";
import { useLibrary } from "@/composables/useLibrary";
import { useVersions } from "@/composables/useVersions";
import {
  installedFromOlder,
  leadOptionLine,
  olderOptionLine,
  olderPlace,
  stackedUnder,
  versionName,
  type DetailLine,
} from "@/services/versions";
import { kindsPresent, formatDate, plural } from "@/services/format";
import { adultnessOf, veils } from "@/services/ledger";
import ConflictMap from "./ConflictMap.vue";
import InstallPlan from "./InstallPlan.vue";
import KindBadge from "./KindBadge.vue";
import MenuTrail from "./MenuTrail.vue";
import PayloadTree from "./PayloadTree.vue";
import RiftButton from "./RiftButton.vue";
import RiftThumb from "./RiftThumb.vue";

const { detail } = defineProps<{ detail: OptionDetail }>();

const world = useWorld();
const nav = useNavigation();
const search = useSearch();
const preflight = usePreflight();
const { settings } = useSettings();
const openPage = () => pageUrl.value && preflight.openUrl(pageUrl.value);

const option = computed(() => detail.option);
const planning = computed(() => world.stage.value !== "detail");
const installedHere = computed(() => detail.installed !== null);

const byLine = computed(() => {
  const info = option.value.info;
  const bits: string[] = [];
  if (info?.author) bits.push(`by ${info.author}`);
  if (info?.version) bits.push(`v${info.version.replace(/^v/i, "")}`);
  if (info?.category) bits.push(info.category);
  return bits.join(" · ");
});

const nexusId = computed(() => detail.package.nexus?.modId ?? null);
const pageUrl = computed(() => detail.update?.pageUrl ?? null);
const labels = computed(() => detail.labels);
const unlabeledLine = computed(() =>
  nexusId.value === null
    ? "No Nexus labels: link this package on the NEXUS page and its category and tags follow."
    : "No Nexus labels yet: the stone learns them at the next update check.",
);
const veiled = computed(() => veils(settings.value.veil, adultnessOf(detail.labels)));

// --- v0.5: the option among the versions of its file ---------------------
const library = useLibrary();
const versions = useVersions();
const familyLine = computed<{ text: string; door: DetailLine | null } | null>(() => {
  const packages = library.packages.value;
  const pkg = packages.find((p) => p.package.id === detail.package.id);
  if (!pkg?.family) return null;
  const lead = versionName(pkg.family.version, pkg.package.fileName);
  const place = olderPlace(packages, pkg.package.id);
  if (place) {
    const leadName = versionName(place.lead.family?.version ?? null, place.lead.package.fileName);
    return { text: olderOptionLine(lead, leadName), door: null };
  }
  const installed = installedHere.value;
  const older = installedFromOlder(pkg, option.value.id, installed) ?? stackedUnder(pkg, option.value.id, installed);
  if (!older) return null;
  const line = leadOptionLine(versionName(older.version, older.fileName), lead, installed);
  return { text: line.text, door: line };
});
const installLabel = computed(() =>
  familyLine.value?.door?.door === "install" ? familyLine.value.door.doorLabel : "Install…",
);

async function keepOnlyTheNewest(): Promise<void> {
  versions.open(detail.package.id);
  await versions.beginKeepNewest(detail.package.id);
}

const update = computed(() => (detail.update?.state.kind === "updateAvailable" ? detail.update.state.newer : null));

// v0.3 (R16.23): the update line's Download door. Premium brings the newer
// file into the mods folder from here; free starts a waiting download and
// opens the file's Nexus page. Cyan, never gold: it lands in the Rift, and the
// install stays the player's next press.
const downloads = useDownloads();
const nexusAccount = useNexusAccount();
const updateDoor = computed(() => {
  const newer = update.value;
  if (!newer || nexusId.value === null || !nexusAccount.connected.value) return null;
  const mine = downloads.forPackage(detail.package.id);
  if (mine && mine.fileId === newer.fileId && mine.phase !== "failed" && mine.phase !== "cancelled") return null;
  let label = "Get it on Nexus";
  if (nexusAccount.premium.value) label = newer.version ? `Download v${newer.version.replace(/^v/i, "")}` : "Download";
  return { modId: nexusId.value, fileId: newer.fileId, label };
});

const conflictSentence = computed(() => {
  const own = option.value.id;
  const wins = detail.conflicts.filter((c) => c.winner.optionId === own);
  const losses = detail.conflicts.filter((c) => c.winner.optionId !== own);
  const parts: string[] = [];
  if (wins.length) parts.push(`overwrites ${plural(wins.length, "file")} of ${otherNames(wins)}`);
  if (losses.length) parts.push(`is overridden on ${plural(losses.length, "file")} by ${otherNames(losses)}`);
  return parts.join("; ");
});

function otherNames(list: OptionDetail["conflicts"]): string {
  const names = new Set<string>();
  for (const c of list) for (const l of c.layers) if (l.optionId !== option.value.id) names.add(l.name);
  return [...names].join(", ");
}

function zoom(): void {
  if (option.value.screenshot) nav.openLightbox({ optionId: option.value.id, name: option.value.name });
}

function crumb(name: string): void {
  nav.openPackage(detail.package.id, name);
}
</script>

<template>
  <article class="odetail" :class="{ 'is-veiled': veiled }">
    <button
      type="button"
      class="odetail__shot"
      :class="{ 'is-strip': planning }"
      :disabled="!option.screenshot"
      :aria-label="option.screenshot ? `Open the screenshot of ${option.name}` : 'No screenshot'"
      @click="zoom"
    >
      <RiftThumb
        :option-id="option.id"
        :has-screenshot="option.screenshot !== null"
        size="full"
        :alt="`Screenshot of ${option.name}`"
      />
    </button>

    <h2 class="odetail__name">{{ option.name }}</h2>
    <MenuTrail :trail="option.trail" :tokens="search.tokens.value" @crumb="crumb" />
    <p class="odetail__by">
      {{ byLine
      }}<template v-if="nexusId !== null"
        ><template v-if="byLine"> · </template
        ><button
          v-if="pageUrl"
          type="button"
          class="odetail__nexus"
          :title="`Open Nexus mod ${nexusId} in your browser`"
          @click="openPage"
        >
          Nexus #{{ nexusId }} ↗</button
        ><template v-else>Nexus #{{ nexusId }}</template></template
      >
      <button v-if="update" type="button" class="odetail__upd" @click="nav.go('nexus')">
        · UPDATE v{{ update.version ?? "?" }} ▸
      </button>
      <RiftButton
        v-if="updateDoor"
        class="odetail__dl"
        size="sm"
        variant="primary"
        :busy="downloads.busy.value === `start:${updateDoor.modId}:${updateDoor.fileId}`"
        @click="downloads.start(updateDoor.modId, updateDoor.fileId, detail.package.id)"
        >{{ updateDoor.label }}</RiftButton
      >
    </p>
    <p v-if="installedHere && detail.installed" class="odetail__in">
      In your world since {{ formatDate(detail.installed.installedAt)
      }}<template v-if="detail.installed.origin === 'adopted'"> · adopted</template
      ><template v-if="detail.installed.gameBuild"> · build {{ detail.installed.gameBuild }}</template>
    </p>
    <div class="odetail__labels" aria-label="Nexus labels">
      <template v-if="labels && (labels.category || labels.tags.length)">
        <button
          v-if="labels.category"
          type="button"
          class="odetail__lab"
          :class="{ 'is-on': search.hasCategory(labels.category) }"
          :aria-pressed="search.hasCategory(labels.category)"
          :title="`Nexus category: ${labels.category}. Press to filter the Rift by it.`"
          @click="search.toggleCategory(labels.category)"
        >
          {{ labels.category }}
        </button>
        <button
          v-for="t in labels.tags"
          :key="t"
          type="button"
          class="odetail__lab odetail__lab--tag"
          :class="{ 'is-on': search.hasTag(t) }"
          :aria-pressed="search.hasTag(t)"
          :title="`Nexus tag: ${t}. Press to filter the Rift by it.`"
          @click="search.toggleTag(t)"
        >
          {{ t }}
        </button>
        <span v-if="labels.adult" class="odetail__adult rs-mono" title="Nexus flags this mod as adult content"
          >adult</span
        >
      </template>
      <span v-else-if="labels?.adult" class="odetail__nolab"
        >Nexus flags this mod as adult content, and gives it no category or tags.</span
      >
      <span v-else class="odetail__nolab">{{ unlabeledLine }}</span>
    </div>
    <p v-if="option.info?.description" class="odetail__desc">{{ option.info.description }}</p>
    <div class="odetail__kinds">
      <KindBadge v-for="k in kindsPresent(option.kinds)" :key="k" :kind="k" :count="option.kinds[k]" />
    </div>

    <p v-if="familyLine && !planning" class="odetail__family">{{ familyLine.text }}</p>

    <div v-if="!planning" class="odetail__doors">
      <RiftButton
        v-if="!installedHere"
        variant="primary"
        :busy="world.planning.value"
        :disabled="option.menuOnly"
        reason="A menu entry installs nothing. Pick one of its variants."
        @click="world.beginInstall()"
        >{{ installLabel }}</RiftButton
      >
      <RiftButton v-if="familyLine?.door?.door === 'keepNewest'" variant="primary" @click="keepOnlyTheNewest()">{{
        familyLine.door.doorLabel
      }}</RiftButton>
      <RiftButton v-else variant="danger" :busy="world.planning.value" @click="world.beginUninstall()"
        >Uninstall…</RiftButton
      >
      <RiftButton @click="nav.openPackage(detail.package.id, option.name)">Open in package</RiftButton>
      <RiftButton @click="preflight.reveal(detail.package.path)">Show in archive</RiftButton>
    </div>

    <InstallPlan v-if="planning" class="odetail__plan" />

    <template v-else>
      <section v-if="detail.conflicts.length" class="odetail__sec">
        <h3 class="odetail__h rs-inscription">CONFLICTS</h3>
        <p class="odetail__conf">⚔ {{ option.name }} {{ conflictSentence }}.</p>
        <ConflictMap :conflicts="detail.conflicts" />
      </section>
      <section v-if="detail.children.length" class="odetail__sec">
        <h3 class="odetail__h rs-inscription">VARIANTS BEHIND THIS MENU</h3>
        <ul class="odetail__kids">
          <li v-for="c in detail.children" :key="c.optionId">
            <button type="button" class="odetail__kid" @click="world.open(c.optionId)">
              <span>{{ c.name }}</span>
              <span class="rs-mono" :class="c.installed ? 'is-world' : 'is-rift'">{{
                c.installed ? "INSTALLED" : c.menuOnly ? "MENU" : ""
              }}</span>
            </button>
          </li>
        </ul>
        <RiftButton size="sm" @click="nav.openLookbook(detail.package.id)">Compare in the lookbook</RiftButton>
      </section>
      <section class="odetail__sec">
        <h3 class="odetail__h rs-inscription">PAYLOAD</h3>
        <PayloadTree :option="option" :tokens="search.tokens.value" />
      </section>
    </template>
  </article>
</template>

<style scoped>
.odetail__shot {
  display: block;
  width: 100%;
  aspect-ratio: 4 / 3;
  padding: 0;
  border: 0;
  background: none;
  cursor: zoom-in;
  transition: aspect-ratio var(--rs-dur-settle) var(--rs-ease-carve);
}
.odetail__shot:disabled {
  cursor: default;
}
.odetail__shot.is-strip {
  aspect-ratio: auto;
  height: 96px;
}
.odetail__shot :deep(.rs-thumb) {
  width: 100%;
  height: 100%;
}
.odetail__name {
  margin: var(--rs-space-4) 0 2px;
  font: 700 var(--rs-text-lg) / 1.2 var(--rs-font-sans);
  overflow-wrap: anywhere;
}
.odetail__by,
.odetail__in {
  margin: 6px 0 0;
  color: var(--rs-faint);
  font-size: var(--rs-text-sm);
}
.odetail__in {
  color: var(--rs-world);
}
.odetail__upd {
  padding: 0;
  border: 0;
  background: none;
  color: var(--rs-verdant);
  font: inherit;
  cursor: pointer;
}
.odetail__dl {
  margin-left: var(--rs-space-2);
  vertical-align: middle;
}
.odetail__nexus {
  padding: 0;
  border: 0;
  background: none;
  color: var(--rs-muted);
  font: inherit;
  text-decoration: underline;
  text-underline-offset: 3px;
  cursor: pointer;
}
.odetail__nexus:hover {
  color: var(--rs-text);
}
/* the NEXUS line: the card's quiet label chip (card__cat), pressable */
.odetail__labels {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  margin-top: var(--rs-space-2);
}
.odetail__lab {
  max-width: 100%;
  min-height: 24px;
  overflow: hidden;
  padding: 1px 6px 1px 8px;
  border: 1px solid var(--rs-edge);
  border-left: 2px solid rgba(143, 227, 242, 0.55);
  border-radius: var(--rs-radius-chip);
  background: transparent;
  color: var(--rs-muted);
  font: 400 var(--rs-text-xs) / 1.3 var(--rs-font-mono);
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
}
.odetail__lab--tag {
  border-left-color: var(--rs-edgehi);
}
.odetail__lab:hover {
  color: var(--rs-text);
  border-color: var(--rs-rift);
}
.odetail__lab.is-on {
  background: var(--rs-riftdeep);
  border-color: var(--rs-rift);
  color: var(--rs-text);
}
.odetail__adult {
  color: var(--rs-faint);
  font-size: var(--rs-text-xs);
  text-transform: uppercase;
  letter-spacing: 0.08em;
}
.odetail__nolab {
  color: var(--rs-faint);
  font-size: var(--rs-text-sm);
}
.odetail__nexus:focus-visible,
.odetail__lab:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: 2px;
}
.odetail__desc {
  margin: var(--rs-space-2) 0 0;
  line-height: 1.45;
}
.odetail__kinds {
  display: flex;
  gap: 6px;
  margin-top: var(--rs-space-3);
}
/* v0.5 (R18.4): the option among its file's versions, under the kinds */
.odetail__family {
  margin: var(--rs-space-3) 0 0;
  color: var(--rs-text);
  font-size: var(--rs-text-sm);
  line-height: 1.45;
}
.odetail__doors {
  display: flex;
  flex-wrap: wrap;
  gap: var(--rs-space-2);
  margin-top: var(--rs-space-4);
}
.odetail__plan {
  margin-top: var(--rs-space-5);
}
.odetail__sec {
  margin-top: var(--rs-space-6);
}
.odetail__h {
  margin: 0 0 var(--rs-space-2);
  font-size: var(--rs-text-sm);
  color: var(--rs-muted);
  letter-spacing: 0.1em;
}
.odetail__conf {
  margin: 0 0 var(--rs-space-3);
  padding: var(--rs-space-2) 10px;
  border-left: 3px solid var(--rs-ember);
  background: var(--rs-hearthhi);
  font-size: var(--rs-text-sm);
}
.odetail__kids {
  margin: 0 0 var(--rs-space-3);
  padding: 0;
  list-style: none;
}
.odetail__kid {
  display: flex;
  justify-content: space-between;
  width: 100%;
  padding: 6px var(--rs-space-2);
  margin-bottom: 2px;
  border: 0;
  background: var(--rs-hearthhi);
  color: var(--rs-text);
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.odetail__kid:hover {
  background: #2b2318;
}
.is-world {
  color: var(--rs-world);
}
.is-rift {
  color: var(--rs-faint);
}
</style>
