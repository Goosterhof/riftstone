// PREFLIGHT's hands: the REFramework switches, the game folder, the library
// roots, and the openers. Each action re-reads the overview through the
// spine's events; failures speak through the voice module as toasts.

import { ref } from "vue";
import type { CheckAction } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { voice, voicedLine, type VoiceContext } from "@/services/voice";
import { useToasts } from "./useToasts";
import { useStone } from "./useStone";
import { useNavigation } from "./useNavigation";

export type ReframeworkSwitch = "looseFileLoader" | "pakDirectory";

export const REFRAMEWORK_URL = "https://www.nexusmods.com/dragonsdogma2/mods/9";
export const NEXUS_KEY_URL = "https://www.nexusmods.com/users/myaccount?tab=api";

const busy = ref<string | null>(null);

async function attempt(id: string, context: VoiceContext, work: () => Promise<void>): Promise<boolean> {
  busy.value = id;
  try {
    await work();
    return true;
  } catch (raw) {
    useToasts().push("ember", voicedLine(voice(asRiftFailure(raw), context)));
    return false;
  } finally {
    busy.value = null;
  }
}

const SWITCH_NAME: Record<ReframeworkSwitch, string> = {
  pakDirectory: "The Alternate PAK Folder",
  looseFileLoader: "The Loose File Loader",
};

async function turnOn(which: ReframeworkSwitch): Promise<void> {
  const ok = await attempt(which, "switch", async () => {
    const backend = await rift();
    await backend.setReframeworkSwitch(which, true);
    await useStone().refreshOverview();
  });
  if (ok) useToasts().push("world", `${SWITCH_NAME[which]} is on. re2_fw_config.txt was backed up first.`);
}

async function pickGameFolder(): Promise<void> {
  const backend = await rift();
  const path = await backend.pickFolder("Where is Dragon's Dogma 2 installed?");
  if (!path) return;
  await attempt("game", "game", async () => {
    await backend.setGameRoot(path);
    await useStone().refreshOverview();
  });
}

async function addRoot(): Promise<void> {
  const backend = await rift();
  const path = await backend.pickFolder("Add a folder where your mod downloads land");
  if (!path) return;
  await attempt("roots", "roots", async () => {
    const root = await backend.addRoot(path);
    await useStone().refreshOverview();
    useToasts().push("rift", `${root.label} joined the library. The stone is reading it now.`);
  });
}

async function removeRoot(rootId: string): Promise<void> {
  await attempt(`root:${rootId}`, "roots", async () => {
    const backend = await rift();
    await backend.removeRoot(rootId);
    await useStone().refreshOverview();
  });
}

async function openUrl(url: string): Promise<void> {
  const backend = await rift();
  await backend.openUrl(url);
}

async function reveal(path: string): Promise<void> {
  const backend = await rift();
  await backend.revealPath(path);
}

async function rescan(): Promise<void> {
  await attempt("scan", "scan", () => useStone().startScan());
}

/** Run a Check's action from the overview. */
async function runCheck(action: CheckAction): Promise<void> {
  const handlers: Record<CheckAction["kind"], () => Promise<void> | void> = {
    adoptFluffy: () => useNavigation().go("world"),
    enableSwitch: () => (action.kind === "enableSwitch" ? turnOn(action.switch) : undefined),
    pickGameFolder: () => pickGameFolder(),
    openUrl: () => (action.kind === "openUrl" ? openUrl(action.url) : undefined),
    rescan: () => rescan(),
  };
  await handlers[action.kind]();
}

export function usePreflight() {
  return { busy, turnOn, pickGameFolder, addRoot, removeRoot, openUrl, reveal, rescan, runCheck };
}
