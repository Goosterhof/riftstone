import { afterEach, expect, it, vi } from "vitest";
import { useWorld } from "@/composables/useWorld";
import { useSearch } from "@/composables/useSearch";
import { useNavigation } from "@/composables/useNavigation";
import { useLibrary } from "@/composables/useLibrary";
import { useVersions } from "@/composables/useVersions";
import type { InstallResultView, UninstallResultView } from "@/types/rift";
import { press, sleep, spine, typeQuery, until, wakeOn } from "./harness";

afterEach(() => {
  vi.restoreAllMocks();
  sleep();
});

function deferred<T>(): { promise: Promise<T>; resolve: (value: T) => void } {
  let settle!: (value: T) => void;
  const promise = new Promise<T>((resolve) => {
    settle = resolve;
  });
  return { promise, resolve: settle };
}

function must<T>(value: T | null | undefined): T {
  if (value === null || value === undefined) throw new Error("The invented fixture is missing");
  return value;
}

it("refreshes an install's status after leaving and returning before it finishes", async () => {
  await wakeOn();
  await typeQuery("ermine");
  const backend = spine();
  const read = backend.getOption.bind(backend);
  const choices = await Promise.all(useSearch().results.value.map((h) => read(h.optionId)));
  const first = must(choices.find((d) => !d.installed));
  const second = must(choices.find((d) => d.option.id !== first.option.id));
  const installed = must(choices.find((d) => d.installed)).installed;
  let committed = false;
  vi.spyOn(backend, "getOption").mockImplementation(async (id) => {
    const detail = await read(id);
    return id === first.option.id ? { ...detail, installed: committed ? installed : null } : detail;
  });
  const finish = deferred<InstallResultView>();
  const install = vi.spyOn(backend, "install").mockImplementation(() => finish.promise);
  const world = useWorld();
  await world.open(first.option.id);
  await world.beginInstall();
  const pending = world.confirmInstall();
  await until(() => install.mock.calls.length === 1, "the install started");
  await world.open(second.option.id);
  await world.open(first.option.id);
  expect(world.isInstalled.value).toBe(false);
  committed = true;
  finish.resolve({
    optionId: first.option.id,
    filesWritten: 1,
    bytesWritten: 1,
    backups: 0,
    durationMs: 1,
    restartNeeded: false,
    putAway: [],
    putAwayFailed: null,
  });
  await pending;
  expect(world.detail.value?.option.id).toBe(first.option.id);
  expect(world.isInstalled.value).toBe(true);
  expect(world.stage.value).toBe("detail");
});

it("refreshes an uninstall's status after leaving and returning before it finishes", async () => {
  await wakeOn();
  await typeQuery("ermine");
  const backend = spine();
  const read = backend.getOption.bind(backend);
  const choices = await Promise.all(useSearch().results.value.map((h) => read(h.optionId)));
  const first = must(choices.find((d) => d.installed));
  const second = must(choices.find((d) => d.option.id !== first.option.id));
  let committed = false;
  vi.spyOn(backend, "getOption").mockImplementation(async (id) => {
    const detail = await read(id);
    return id === first.option.id ? { ...detail, installed: committed ? null : first.installed } : detail;
  });
  const finish = deferred<UninstallResultView>();
  const uninstall = vi.spyOn(backend, "uninstall").mockImplementation(() => finish.promise);
  const world = useWorld();
  await world.open(first.option.id);
  await world.beginUninstall();
  const pending = world.confirmUninstall();
  await until(() => uninstall.mock.calls.length === 1, "the uninstall started");
  await world.open(second.option.id);
  await world.open(first.option.id);
  expect(world.isInstalled.value).toBe(true);
  committed = true;
  finish.resolve({
    optionId: first.option.id,
    filesRemoved: 1,
    filesRestored: 0,
    filesKept: 0,
    filesHandedDown: 0,
    parked: 0,
    restartNeeded: false,
  });
  await pending;
  expect(world.detail.value?.option.id).toBe(first.option.id);
  expect(world.isInstalled.value).toBe(false);
  expect(world.stage.value).toBe("detail");
});

it("keeps room shortcuts in the screenshot viewer even after focus leaves its controls", async () => {
  await wakeOn();
  const nav = useNavigation();
  nav.openLightbox({ optionId: "ermine-var-1", name: "Ermine Cape" });
  await until(() => document.activeElement?.classList.contains("lightbox__x") === true, "the viewer focused");
  (document.activeElement as HTMLElement).blur();
  press("4", {}, document.body);
  expect(nav.lightbox.value).not.toBeNull();
  expect(nav.page.value).toBe("rift");
});

it("closes only the screenshot over a versions card when Escape starts outside its controls", async () => {
  await wakeOn("family");
  await until(() => useLibrary().packages.value.some((p) => p.family?.older.length), "the family");
  const lead = must(useLibrary().packages.value.find((p) => p.family?.older.length));
  const versions = useVersions();
  versions.open(lead.package.id);
  await until(() => document.querySelector(".vc") !== null, "the versions card");
  const nav = useNavigation();
  nav.openLightbox({ optionId: must(lead.tree[0].optionId), name: lead.tree[0].label });
  await until(() => document.activeElement?.classList.contains("lightbox__x") === true, "the viewer focused");
  (document.activeElement as HTMLElement).blur();
  press("Escape", {}, document.body);
  await until(() => nav.lightbox.value === null, "the viewer closed");
  expect(versions.leadId.value).toBe(lead.package.id);
});

it("keeps focus inside the viewer after its non-focusable picture is clicked", async () => {
  await wakeOn();
  const nav = useNavigation();
  nav.openLightbox({ optionId: "ermine-var-1", name: "Ermine Cape" });
  await until(() => document.activeElement?.classList.contains("lightbox__x") === true, "the viewer focused");
  (document.activeElement as HTMLElement).blur();
  must(document.querySelector<HTMLElement>(".lightbox__img")).click();
  expect(document.activeElement?.closest(".lightbox")).not.toBeNull();
});
