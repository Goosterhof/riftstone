// THE nxm:// HANDLER (v0.3): which program Windows hands Nexus's Mod Manager
// Download links to (get_nxm_handler), and the player's switch that makes the
// stone that program, or gives the links back (set_nxm_handler). Off by
// default: turning it on is the player's act, and turning it off restores the
// program it replaced, or nobody (WINDOW-CONTRACT § v0.3 The nxm handler).
// A free account's downloads arrive through it.

import { computed, ref } from "vue";
import type { NxmHandlerView } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { voice, voicedLine, type Voiced } from "@/services/voice";
import { useToasts } from "./useToasts";

const handler = ref<NxmHandlerView | null>(null);
const busy = ref(false);
const failure = ref<Voiced | null>(null);

/** The stone answers nxm:// links now (the contract's state "ours"). */
const answering = computed(() => handler.value?.state === "ours");
/** This PC can change the handler at all (Windows; the bench answers "unsupported"). */
const supported = computed(() => handler.value !== null && handler.value.state !== "unsupported");

async function load(): Promise<void> {
  try {
    const backend = await rift();
    handler.value = await backend.getNxmHandler();
    failure.value = null;
  } catch (raw) {
    failure.value = voice(asRiftFailure(raw), "nxm");
  }
}

/** What the switch did, said once. `gaveBackTo` is the program the stone had
 *  replaced, read before it was turned off. */
function switchedLine(on: boolean, gaveBackTo: string | null): string {
  if (on) return "The stone answers nxm:// links now: Mod Manager Download on a Nexus file page sends the file here.";
  return gaveBackTo
    ? `nxm:// links go back to ${gaveBackTo}.`
    : "No program answers nxm:// links now; the stone stopped answering them.";
}

async function set(on: boolean): Promise<boolean> {
  busy.value = true;
  failure.value = null;
  const gaveBackTo = handler.value?.givesBackTo ?? null;
  try {
    const backend = await rift();
    handler.value = await backend.setNxmHandler(on);
    useToasts().push("rift", switchedLine(on, gaveBackTo));
    return true;
  } catch (raw) {
    failure.value = voice(asRiftFailure(raw), "nxm");
    useToasts().push("ember", voicedLine(failure.value));
    return false;
  } finally {
    busy.value = false;
  }
}

export function useNxmHandler() {
  return { handler, busy, failure, answering, supported, load, set };
}

export function resetNxmHandler(): void {
  handler.value = null;
  busy.value = false;
  failure.value = null;
}
