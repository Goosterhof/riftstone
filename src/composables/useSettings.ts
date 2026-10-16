// The two switches the user owns (veil, auto-check). Persisted by the spine.

import { ref } from "vue";
import type { Settings } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { voice, voicedLine } from "@/services/voice";
import { useToasts } from "./useToasts";

const settings = ref<Settings>({ veilThumbnails: false, autoCheckUpdates: true });

async function loadSettings(): Promise<void> {
  const backend = await rift();
  settings.value = await backend.getSettings();
}

async function saveSettings(next: Settings): Promise<void> {
  const before = settings.value;
  settings.value = next;
  try {
    const backend = await rift();
    settings.value = await backend.setSettings(next);
  } catch (raw) {
    settings.value = before;
    useToasts().push("ember", voicedLine(voice(asRiftFailure(raw), "settings")));
  }
}

export function useSettings() {
  return {
    settings,
    loadSettings,
    setVeil: (on: boolean) => saveSettings({ ...settings.value, veilThumbnails: on }),
    setAutoCheck: (on: boolean) => saveSettings({ ...settings.value, autoCheckUpdates: on }),
  };
}

export function resetSettings(): void {
  settings.value = { veilThumbnails: false, autoCheckUpdates: true };
}
