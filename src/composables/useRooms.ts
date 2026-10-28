// The rooms past the five every build has. v0.7's sixth exists only in the
// investor's build (RS-10, #00015 R20.1): the import below sits behind
// `import.meta.env.VITE_RIFT_PAWN === "1"`, which Vite replaces with a constant,
// so without the flag the branch is dead and the room never reaches dist/.
// The public window holds only this registry; the row's words, its key and its
// page all come from the gated module.

import { shallowRef, type Component, type Ref } from "vue";
import type { Page } from "./useNavigation";

/** A carved suffix on the room's nav row while it holds something (R20.1). */
export interface RoomSuffix {
  text: string;
  tone: "rift" | "warn";
}

export interface ExtraRoom {
  id: Page;
  label: string;
  hint: string;
  key: string;
  page: Component;
  suffix: Ref<RoomSuffix | null>;
}

const extra = shallowRef<ExtraRoom[]>([]);

/** Loads the gated rooms once. A build without the flag loads none. */
export async function loadExtraRooms(): Promise<void> {
  if (import.meta.env.VITE_RIFT_PAWN === "1") {
    const { workshopRoom } = await import("@/workshop/room");
    extra.value = [workshopRoom()];
  }
}

export function useRooms() {
  return { extra };
}
