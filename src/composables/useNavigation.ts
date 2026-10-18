// Where the user stands: which room right of the stone, and — in the Rift —
// whether it shows answers, one package's tree, or the lookbook.

import { ref } from "vue";

export type Page = "rift" | "world" | "nexus" | "preflight" | "muster";
export type RiftMode = "results" | "package" | "lookbook";

export interface LightboxImage {
  optionId: string;
  name: string;
}

const page = ref<Page>("rift");
const riftMode = ref<RiftMode>("results");
const packageId = ref<string | null>(null);
/** The tree node the package view scrolls to (a crumb click). */
const focusNodeName = ref<string | null>(null);
const lightbox = ref<LightboxImage | null>(null);

function go(next: Page): void {
  page.value = next;
}

function openPackage(id: string, nodeName: string | null = null): void {
  page.value = "rift";
  packageId.value = id;
  focusNodeName.value = nodeName;
  riftMode.value = "package";
}

function openLookbook(id: string): void {
  page.value = "rift";
  packageId.value = id;
  riftMode.value = "lookbook";
}

function backToResults(): void {
  riftMode.value = "results";
  focusNodeName.value = null;
}

function openLightbox(image: LightboxImage): void {
  lightbox.value = image;
}

function closeLightbox(): void {
  lightbox.value = null;
}

export function useNavigation() {
  return {
    page,
    riftMode,
    packageId,
    focusNodeName,
    lightbox,
    go,
    openPackage,
    openLookbook,
    backToResults,
    openLightbox,
    closeLightbox,
  };
}

export function resetNavigation(): void {
  page.value = "rift";
  riftMode.value = "results";
  packageId.value = null;
  focusNodeName.value = null;
  lightbox.value = null;
}
