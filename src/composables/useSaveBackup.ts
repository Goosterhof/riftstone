// A manual snapshot belongs to all saves, independent of the character being read.
import { ref } from "vue";
import { asRiftFailure, rift } from "@/services/rift";
import type { SaveBackupReport } from "@/types/rift";

function describe(raw: unknown): string {
  const failure = asRiftFailure(raw);
  return (
    failure.detail.reason ||
    failure.detail.detail ||
    "The backup could not be made. Choose another folder and try again."
  );
}

export function useSaveBackup() {
  const busy = ref(false);
  const report = ref<SaveBackupReport | null>(null);
  const error = ref<string | null>(null);

  async function create(): Promise<void> {
    if (busy.value) return;
    busy.value = true;
    error.value = null;
    try {
      const backend = await rift();
      const destination = await backend.pickFolder("Choose a folder for the DD2 save backup");
      if (!destination) return;
      report.value = null;
      report.value = await backend.backupSaves(destination);
    } catch (e) {
      error.value = describe(e);
    } finally {
      busy.value = false;
    }
  }

  async function show(): Promise<void> {
    if (!report.value) return;
    try {
      await (await rift()).revealPath(report.value.path);
    } catch {
      error.value = "The backup is saved, but Explorer could not show it. Open the folder you chose to find it.";
    }
  }
  return { busy, report, error, create, show };
}
