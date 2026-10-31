// THE WORKSHOP's registration (#00015 R20.1): the sixth room, key 6, after
// THE MUSTER. Imported only by useRooms under VITE_RIFT_PAWN=1 (RS-10), so
// these words exist only in the investor's build.

import { computed, markRaw } from "vue";
import type { ExtraRoom, RoomSuffix } from "@/composables/useRooms";
import WorkshopPage from "./WorkshopPage.vue";
import { useWorkshop } from "./useWorkshop";

export function workshopRoom(): ExtraRoom {
  const w = useWorkshop();
  void w.subscribe().then(() => w.load());
  const suffix = computed<RoomSuffix | null>(() => {
    // R20.37: any held ask (update or Play) says ASKS; two say how many.
    const asks = (w.held.value ? 1 : 0) + (w.heldUpdate.value ? 1 : 0);
    if (asks === 2) return { text: "Tamsin asks 2 things", tone: "warn" };
    if (w.heldUpdate.value) return { text: "Tamsin asks to update", tone: "warn" };
    if (w.held.value) return { text: "Tamsin asks to play", tone: "warn" };
    if (w.here.value) return { text: "Tamsin is here", tone: "rift" };
    return null;
  });
  return {
    id: "workshop",
    label: "THE WORKSHOP",
    hint: "Build mods with the pawn",
    key: "6",
    page: markRaw(WorkshopPage),
    suffix,
  };
}
