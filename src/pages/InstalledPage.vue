<script setup lang="ts">
// §8.4 — YOUR WORLD, the whole right of the stone warm. The load order (480)
// beside everything else (552): loose/script/root installs, who overrides
// whom, REFramework's proof that the paks loaded, strays, missing files, and
// Fluffy's installs waiting to be adopted — shown as a plan before writing.
import { computed, onMounted } from "vue";
import { useInstalled } from "@/composables/useInstalled";
import { useStone } from "@/composables/useStone";
import { usePreflight, REFRAMEWORK_URL } from "@/composables/usePreflight";
import { formatBytes, formatDate, kindsPresent, plural } from "@/services/format";
import { missingPackageTitle } from "@/services/voice";
import ConflictMap from "@/components/ConflictMap.vue";
import FailureNotice from "@/components/FailureNotice.vue";
import KindBadge from "@/components/KindBadge.vue";
import LoadOrderList from "@/components/LoadOrderList.vue";
import RiftButton from "@/components/RiftButton.vue";
import RiftInscription from "@/components/RiftInscription.vue";

const installed = useInstalled();
const stone = useStone();
const preflight = usePreflight();

const view = computed(() => installed.view.value);
const game = computed(() => stone.overview.value?.game ?? null);
const fluffy = computed(() => view.value?.fluffyUnadopted ?? []);
const pakCount = computed(() => installed.pakRows.value.reduce((n, r) => n + r.paks.length, 0));

const pakLoadLine = computed(() => {
  const report = view.value?.pakLoad;
  if (!report) return "No REFramework log yet: launch the game once and the stone can prove which paks loaded.";
  const started = report.logStarted ? `The game last started ${formatDate(report.logStarted)}` : "The last launch";
  const n = report.injected.length;
  if (report.error) return `${started}. REFramework reported: “${report.error}”`;
  return `${started}; REFramework injected ${plural(n, "pak")} from pak_mods/.`;
});

onMounted(() => void installed.load());
</script>

<template>
  <main class="home" aria-label="Your World — installed">
    <header class="home__head">
      <div>
        <h1 class="home__title rs-inscription">YOUR WORLD</h1>
        <p class="home__sub rs-mono">
          {{ plural(view?.installs.length ?? 0, "mod")
          }}<template v-if="game"> · build {{ game.buildId ?? "unknown" }}</template>
        </p>
      </div>
      <RiftButton v-if="fluffy.length" variant="primary" @click="installed.openAdoption()"
        >Adopt Fluffy's installs</RiftButton
      >
    </header>

    <FailureNotice
      v-if="installed.loadFailure.value"
      :voiced="installed.loadFailure.value"
      @recover="installed.load()"
    />

    <div v-if="stone.buildChange.value" class="home__banner">
      <RiftInscription
        size="md"
        tone="warn"
        inscription="YOUR WORLD HAS CHANGED"
        :body="`Dragon's Dogma 2 updated from build ${stone.buildChange.value.recorded} to ${stone.buildChange.value.now} since your mods were installed. Most still work; script mods and REFramework are the first to break.`"
      >
        <RiftButton size="sm" variant="primary" @click="preflight.openUrl(REFRAMEWORK_URL)"
          >Check REFramework</RiftButton
        >
        <RiftButton size="sm" @click="stone.dismissBuildChange()">I've tested it, dismiss</RiftButton>
      </RiftInscription>
    </div>

    <section v-if="installed.adoptStage.value !== 'closed'" class="home__adopt" aria-label="Adopt Fluffy's installs">
      <h2 class="home__h rs-inscription">FLUFFY'S INSTALLS — THE ADOPTION</h2>
      <template v-if="installed.adoptStage.value === 'done' && installed.adoptResult.value">
        <p class="home__line is-world">
          Adopted {{ plural(installed.adoptResult.value.adopted, "install") }}.
          {{ installed.adoptResult.value.alreadyAdopted }} were already the stone's.
          <template v-if="installed.adoptResult.value.missingFiles">
            {{ plural(installed.adoptResult.value.missingFiles, "file") }} Fluffy listed were already gone.</template
          >
          <template v-if="installed.adoptResult.value.unmatched.length">
            Adopted under their Fluffy names: {{ installed.adoptResult.value.unmatched.join(", ") }}.</template
          >
          Fluffy's own files were not changed.
        </p>
        <RiftButton size="sm" @click="installed.closeAdoption()">Done</RiftButton>
      </template>
      <template v-else>
        <p class="home__line">
          The stone reads Fluffy's installed.ini and takes each install into its own journal, so you can uninstall and
          reorder it here. Nothing is moved and Fluffy's files are not changed.
        </p>
        <ul class="home__rows">
          <li v-for="f in fluffy" :key="f.section" class="home__row">
            <span class="home__name">{{ f.name }}</span>
            <span class="rs-mono home__dim">[{{ f.section }}] · {{ plural(f.fileCount, "file") }}</span>
            <span class="home__dim">{{
              f.matchedOptionId ? "matches a library option" : "no library match: adopted under its Fluffy name"
            }}</span>
          </li>
        </ul>
        <div class="home__acts">
          <RiftButton
            variant="world"
            :busy="installed.adoptStage.value === 'adopting'"
            @click="installed.confirmAdopt()"
            >Adopt {{ plural(fluffy.length, "install") }}</RiftButton
          >
          <RiftButton @click="installed.closeAdoption()">Cancel</RiftButton>
        </div>
      </template>
    </section>

    <div class="home__split">
      <section class="home__left">
        <header class="home__colhead">
          <h2 class="home__h rs-inscription">THE LOAD ORDER — <span class="home__later">later wins</span></h2>
          <div v-if="installed.orderDirty.value" class="home__acts">
            <RiftButton size="sm" variant="world" :busy="installed.applying.value" @click="installed.applyOrder()"
              >Apply order</RiftButton
            >
            <RiftButton size="sm" @click="installed.resetOrder()">Reset</RiftButton>
          </div>
        </header>
        <LoadOrderList />
        <p v-if="view && pakCount > view.pakCountWarning" class="home__warn">
          ⚠ {{ pakCount }} paks in pak_mods/. Above {{ view.pakCountWarning }}, REFramework's slot table starts to
          overflow.
        </p>

        <h2 class="home__h rs-inscription home__gap">PAK LOAD PROOF</h2>
        <p class="home__line" :class="{ 'is-ember': view?.pakLoad?.error }">{{ pakLoadLine }}</p>
        <ul v-if="view?.pakLoad?.injected.length" class="home__rows">
          <li v-for="p in view.pakLoad.injected" :key="p.fileName" class="home__row home__row--proof rs-mono">
            <span class="home__name">{{ p.fileName }}</span>
            <span class="home__dim">slot {{ p.slot }} · {{ p.nativeName }}</span>
          </li>
        </ul>
      </section>

      <section class="home__right">
        <h2 class="home__h rs-inscription">LOOSE · SCRIPT · ROOT</h2>
        <p v-if="!installed.otherRows.value.length" class="home__line home__dim">
          No loose, script or root mods in your game.
        </p>
        <ul class="home__rows">
          <li v-for="r in installed.otherRows.value" :key="r.optionId" class="home__row home__row--install">
            <span class="home__name">{{ r.name }}</span>
            <KindBadge v-for="k in kindsPresent(r.kinds)" :key="k" :kind="k" />
            <span class="rs-mono home__dim">{{ plural(r.fileCount, "file") }}</span>
            <span v-if="r.origin === 'fluffy'" class="rs-mono home__dim">via Fluffy</span>
            <span v-if="r.packageMissing" class="rs-mono is-warn" :title="missingPackageTitle(r)">no package</span>
            <RiftButton
              size="sm"
              variant="danger"
              :aria-label="`Uninstall ${r.name}`"
              @click="installed.planReturn(r.optionId)"
              >−</RiftButton
            >
          </li>
        </ul>

        <div v-if="installed.returning.value" class="home__return">
          <h3 class="home__h rs-inscription">
            THE RETURN — {{ plural(installed.returning.value.removals.length, "file") }} leave your game
          </h3>
          <p v-if="installed.returning.value.refused" class="home__line is-ember">
            {{ installed.returning.value.refused }}
          </p>
          <ul class="home__rows rs-mono">
            <li v-for="r in installed.returning.value.removals" :key="r.gamePath" class="home__row">
              <span class="home__name">{{ r.gamePath }}</span>
              <span :class="r.action.kind === 'restoreBackup' ? 'is-world' : 'home__dim'">{{
                r.action.kind === "restoreBackup" ? `↩ restores ${r.action.owner}'s file` : "− removed"
              }}</span>
            </li>
          </ul>
          <div class="home__acts">
            <RiftButton
              variant="danger"
              :busy="installed.returnBusy.value"
              :disabled="installed.returning.value.refused !== null"
              @click="installed.confirmReturn()"
              >Uninstall {{ installed.returning.value.name }}</RiftButton
            >
            <RiftButton @click="installed.cancelReturn()">Cancel</RiftButton>
          </div>
        </div>

        <h2 class="home__h rs-inscription home__gap">WHO OVERRIDES WHOM</h2>
        <ConflictMap :conflicts="view?.conflicts ?? []" />

        <template v-if="view?.missing.length">
          <h2 class="home__h rs-inscription home__gap">MISSING</h2>
          <ul class="home__rows rs-mono">
            <li v-for="m in view.missing" :key="m.gamePath" class="home__row">
              <span class="home__name">{{ m.gamePath }}</span
              ><span class="is-warn">{{ m.optionName }}'s file is gone</span>
            </li>
          </ul>
        </template>

        <template v-if="view?.strays.length">
          <h2 class="home__h rs-inscription home__gap">STRAYS</h2>
          <p class="home__line home__dim">
            Files in the mod folders that no manager put there. The stone leaves them alone.
          </p>
          <ul class="home__rows rs-mono">
            <li v-for="s in view.strays" :key="s.gamePath" class="home__row">
              <span class="home__name">{{ s.gamePath }}</span
              ><span class="home__dim">{{ formatBytes(s.size) }}</span>
            </li>
          </ul>
        </template>

        <template v-if="fluffy.length && installed.adoptStage.value === 'closed'">
          <h2 class="home__h rs-inscription home__gap">FLUFFY'S INSTALLS</h2>
          <p class="home__line">
            Fluffy installed {{ plural(fluffy.length, "mod") }} the stone does not manage yet:
            {{ fluffy.map((f) => f.name).join(", ") }}.
          </p>
          <RiftButton size="sm" variant="primary" @click="installed.openAdoption()">Adopt Fluffy's installs</RiftButton>
        </template>
      </section>
    </div>
  </main>
</template>

<style scoped>
.home {
  flex: 1;
  min-width: 0;
  height: 100%;
  overflow-y: auto;
  padding: var(--rs-space-5) var(--rs-space-6) var(--rs-space-12);
  background: var(--rs-hearth);
}
.home__head {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  gap: var(--rs-space-4);
  padding-bottom: var(--rs-space-4);
  border-bottom: 1px solid var(--rs-edge);
}
.home__title {
  margin: 0;
  font-size: var(--rs-text-xl);
  line-height: 1.15;
}
.home__sub {
  margin: 4px 0 0;
  color: var(--rs-faint);
}
.home__banner,
.home__adopt {
  margin-top: var(--rs-space-4);
}
.home__adopt {
  padding: var(--rs-space-4);
  border-left: 3px solid var(--rs-world);
  background: var(--rs-hearthhi);
}
.home__split {
  display: grid;
  grid-template-columns: minmax(0, 480fr) minmax(0, 552fr);
  gap: var(--rs-space-6);
  margin-top: var(--rs-space-5);
}
@media (max-width: 1199px) {
  .home__split {
    grid-template-columns: minmax(0, 1fr);
  }
}
.home__colhead {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--rs-space-3);
  min-height: 32px;
  margin-bottom: var(--rs-space-2);
}
.home__h {
  margin: 0 0 var(--rs-space-2);
  font-size: 14px;
  color: var(--rs-muted);
  letter-spacing: 0.1em;
}
.home__colhead .home__h {
  margin: 0;
}
.home__later {
  font-family: var(--rs-font-sans);
  font-weight: 500;
  letter-spacing: 0;
  text-transform: none;
  color: var(--rs-faint);
}
.home__gap {
  margin-top: var(--rs-space-6);
}
.home__line {
  margin: 0 0 var(--rs-space-3);
  line-height: 1.5;
}
.home__rows {
  margin: 0 0 var(--rs-space-3);
  padding: 0;
  list-style: none;
}
.home__row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 4px var(--rs-space-3);
  min-height: 34px;
  padding: 4px var(--rs-space-3);
  margin-bottom: 2px;
  background: var(--rs-hearthhi);
}
.home__row--install {
  flex-wrap: nowrap;
}
.home__name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.home__dim {
  color: var(--rs-faint);
}
/* PAK LOAD PROOF: the pak's own name is the datum, so it keeps its full width
   and the slot line wraps under it instead of squeezing it to "zrif…". */
.home__row--proof .home__name {
  flex: 1 0 auto;
  max-width: 100%;
}
.home__row--proof .home__dim {
  min-width: 0;
  overflow-wrap: anywhere;
}
.home__acts {
  display: flex;
  gap: var(--rs-space-2);
}
.home__return {
  margin: var(--rs-space-3) 0;
  padding: var(--rs-space-4);
  border-left: 3px solid var(--rs-ember);
  background: rgba(35, 28, 19, 0.6);
}
.home__warn {
  margin: var(--rs-space-3) 0 0;
  color: var(--rs-warn);
}
.is-world {
  color: var(--rs-world);
}
.is-warn {
  color: var(--rs-warn);
}
.is-ember {
  color: var(--rs-ember);
}
</style>
