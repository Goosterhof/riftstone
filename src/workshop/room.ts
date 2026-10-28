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
    if (w.held.value) return { text: "ASKS", tone: "warn" };
    if (w.here.value) return { text: "HERE", tone: "rift" };
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
