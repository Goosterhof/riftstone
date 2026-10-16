// The two choices the user owns (the veil: off / adult / all, and the
// auto-check). Persisted by the spine, which reads v0.1's veilThumbnails.

import { ref } from "vue";
import type { Settings, VeilMode } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { voice, voicedLine } from "@/services/voice";
import { useToasts } from "./useToasts";

const settings = ref<Settings>({ veil: "off", autoCheckUpdates: true });

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
    setVeil: (veil: VeilMode) => saveSettings({ ...settings.value, veil }),
    setAutoCheck: (on: boolean) => saveSettings({ ...settings.value, autoCheckUpdates: on }),
  };
}

export function resetSettings(): void {
  settings.value = { veil: "off", autoCheckUpdates: true };
}
