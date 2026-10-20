import { afterEach, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { useWorld } from "@/composables/useWorld";
import { useSearch } from "@/composables/useSearch";
import { useNavigation } from "@/composables/useNavigation";
import { useLibrary } from "@/composables/useLibrary";
import { useVersions } from "@/composables/useVersions";
import { useToasts } from "@/composables/useToasts";
import type { InstallResultView, UninstallResultView } from "@/types/rift";
import { button, press, sleep, spine, typeQuery, until, wakeOn } from "./harness";

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

function holdWrite(act: "install" | "uninstall", ready: Promise<void>, committed: () => void = () => undefined) {
  const backend = spine();
  async function hold<T>(write: () => Promise<T>): Promise<T> {
    await ready;
    const result = await write();
    committed();
    return result;
  }
  if (act === "install") {
    const install = backend.install.bind(backend);
    return vi.spyOn(backend, "install").mockImplementation((id) => hold(() => install(id)));
  }
  const uninstall = backend.uninstall.bind(backend);
  return vi.spyOn(backend, "uninstall").mockImplementation((id) => hold(() => uninstall(id)));
}

/** Leave a toast whose Undo performs `act`, then return to the option's detail. */
async function prepareUndo(act: "install" | "uninstall") {
  await wakeOn();
  await typeQuery("ermine");
  const backend = spine();
  const choices = await Promise.all(useSearch().results.value.map((h) => backend.getOption(h.optionId)));
  const first = must(choices.find((d) => !d.option.menuOnly && Boolean(d.installed) === (act === "install")));
  const world = useWorld();
  await world.open(first.option.id);
  await (act === "install" ? world.beginUninstall() : world.beginInstall());
  await (act === "install" ? world.confirmUninstall() : world.confirmInstall());
  const undo = must(
    useToasts()
      .toasts.value.flatMap((t) => t.actions)
      .find((a) => a.label === "Undo"),
  );
  world.backToDetail();
  return { world, undo, optionId: first.option.id };
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

it.each(["install", "uninstall"] as const)(
  "keeps a revisited %s door busy until its write and refresh finish",
  async (act) => {
    await wakeOn();
    await typeQuery("ermine");
    const backend = spine();
    const choices = await Promise.all(useSearch().results.value.map((h) => backend.getOption(h.optionId)));
    const first = must(choices.find((d) => !d.option.menuOnly && Boolean(d.installed) === (act === "uninstall")));
    const second = must(choices.find((d) => !d.option.menuOnly && d.option.id !== first.option.id));
    const finish = deferred<void>();
    const readAgain = deferred<void>();
    let refreshing = false;
    const read = backend.getOption.bind(backend);
    vi.spyOn(backend, "getOption").mockImplementation(async (id) => {
      if (refreshing && id === first.option.id) await readAgain.promise;
      return read(id);
    });
    const writes = holdWrite(act, finish.promise, () => {
      refreshing = true;
    });
    const plans = act === "install" ? vi.spyOn(backend, "planInstall") : vi.spyOn(backend, "planUninstall");
    const world = useWorld();
    const begin = act === "install" ? world.beginInstall : world.beginUninstall;
    const confirm = act === "install" ? world.confirmInstall : world.confirmUninstall;
    const label = act === "install" ? "Install…" : "Uninstall…";
    await world.open(first.option.id);
    await begin();
    const pending = confirm();
    await until(() => writes.mock.calls.length === 1, "the write started");
    await world.open(second.option.id);
    await until(() => document.querySelector(".odetail__doors") !== null, "the other option's doors");
    expect(button(world.isInstalled.value ? "Uninstall…" : "Install…").getAttribute("aria-busy")).toBeNull();
    await world.open(first.option.id);
    await until(() => document.querySelector(".odetail__doors") !== null, "the revisited doors");
    expect.soft(button(label).getAttribute("aria-busy")).toBe("true");
    button(label).click();
    await begin(); // The command guard must also hold when no button is involved.
    expect.soft(plans).toHaveBeenCalledTimes(1);
    finish.resolve();
    await until(() => refreshing, "the write finished before its detail refresh");
    expect.soft(button(label).getAttribute("aria-busy")).toBe("true");
    await begin();
    expect.soft(plans).toHaveBeenCalledTimes(1);
    readAgain.resolve();
    await pending;
    expect(writes).toHaveBeenCalledTimes(1);
    expect(world.isInstalled.value).toBe(act === "install");
    expect(world.stage.value).toBe("detail");
    expect(button(act === "install" ? "Uninstall…" : "Install…").getAttribute("aria-busy")).toBeNull();
  },
);

it.each(["install", "uninstall"] as const)("releases a revisited door when its %s fails", async (act) => {
  await wakeOn();
  await typeQuery("ermine");
  const backend = spine();
  const choices = await Promise.all(useSearch().results.value.map((h) => backend.getOption(h.optionId)));
  const first = must(choices.find((d) => !d.option.menuOnly && Boolean(d.installed) === (act === "uninstall")));
  const second = must(choices.find((d) => d.option.id !== first.option.id));
  const finish = deferred<void>();
  const write = vi.spyOn(backend, act).mockImplementationOnce(async () => {
    await finish.promise;
    throw new Error("Invented write failure");
  });
  const world = useWorld();
  const begin = act === "install" ? world.beginInstall : world.beginUninstall;
  const confirm = act === "install" ? world.confirmInstall : world.confirmUninstall;
  await world.open(first.option.id);
  await begin();
  const pending = confirm();
  await until(() => write.mock.calls.length === 1, "the write started");
  await world.open(second.option.id);
  await world.open(first.option.id);
  finish.resolve();
  await pending;
  expect(button(act === "install" ? "Install…" : "Uninstall…").getAttribute("aria-busy")).toBeNull();
  await begin();
  expect(world.stage.value).toBe(act === "install" ? "plan" : "returnPlan");
});

it.each(["install", "uninstall"] as const)(
  "owns the option while undoing an %s, including a repeated Undo",
  async (act) => {
    await wakeOn();
    await typeQuery("ermine");
    const backend = spine();
    const choices = await Promise.all(useSearch().results.value.map((h) => backend.getOption(h.optionId)));
    const first = must(choices.find((d) => !d.option.menuOnly && Boolean(d.installed) === (act === "uninstall")));
    const world = useWorld();
    await world.open(first.option.id);
    await (act === "install" ? world.beginInstall() : world.beginUninstall());
    await (act === "install" ? world.confirmInstall() : world.confirmUninstall());
    const undo = must(
      useToasts()
        .toasts.value.flatMap((t) => t.actions)
        .find((a) => a.label === "Undo"),
    );
    const finish = deferred<void>();
    const undoAct = act === "install" ? "uninstall" : "install";
    const writes = holdWrite(undoAct, finish.promise);
    const pending = undo.run();
    await until(() => writes.mock.calls.length === 1, "Undo started");
    const repeated = undo.run();
    await flushPromises();
    expect.soft(writes).toHaveBeenCalledTimes(1);
    world.close();
    await world.open(first.option.id);
    expect.soft(button(act === "install" ? "Uninstall…" : "Install…").getAttribute("aria-busy")).toBe("true");
    finish.resolve();
    await pending;
    await repeated;
    expect(world.isInstalled.value).toBe(act === "uninstall");
    expect(button(act === "install" ? "Install…" : "Uninstall…").getAttribute("aria-busy")).toBeNull();
  },
);

it.each(["install", "uninstall"] as const)("refuses a drawn %s plan while Undo already owns its write", async (act) => {
  const { world, undo } = await prepareUndo(act);
  await (act === "install" ? world.beginInstall() : world.beginUninstall());
  expect(world.stage.value).toBe(act === "install" ? "plan" : "returnPlan");
  const finish = deferred<void>();
  const writes = holdWrite(act, finish.promise);
  const pendingUndo = undo.run();
  await until(() => writes.mock.calls.length === 1, "Undo owns the write");
  const pendingConfirm = act === "install" ? world.confirmInstall() : world.confirmUninstall();
  await flushPromises();
  expect.soft(writes).toHaveBeenCalledTimes(1);
  finish.resolve();
  await Promise.all([pendingUndo, pendingConfirm]);
  expect(writes).toHaveBeenCalledTimes(1);
  expect(world.isInstalled.value).toBe(act === "install");
});

it("ignores a late install plan while Undo owns its option", async () => {
  const { world, undo, optionId } = await prepareUndo("install");
  const backend = spine();
  const answer = await backend.planInstall(optionId);
  const finishPlan = deferred<void>();
  const plans = vi.spyOn(backend, "planInstall").mockImplementation(async () => {
    await finishPlan.promise;
    return answer;
  });
  const pendingPlan = world.beginInstall();
  await until(() => plans.mock.calls.length === 1, "the plan was requested");
  const finishWrite = deferred<void>();
  const writes = holdWrite("install", finishWrite.promise);
  const pendingUndo = undo.run();
  await until(() => writes.mock.calls.length === 1, "Undo owns the option");
  finishPlan.resolve();
  await pendingPlan;
  expect.soft(world.plan.value).toBeNull();
  expect.soft(world.stage.value).toBe("detail");
  finishWrite.resolve();
  await pendingUndo;
  expect(world.isInstalled.value).toBe(true);
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
  expect(document.activeElement?.closest(".lightbox")).not.toBeNull();
});

it("keeps keys from later capture listeners on the same window while a screenshot is open", async () => {
  await wakeOn();
  const nav = useNavigation();
  nav.openLightbox({ optionId: "ermine-var-1", name: "Ermine Cape" });
  await until(() => document.activeElement?.classList.contains("lightbox__x") === true, "the viewer focused");
  const behind = vi.fn<() => void>();
  window.addEventListener("keydown", behind, { capture: true });
  try {
    press("4");
    expect(behind).not.toHaveBeenCalled();
    nav.closeLightbox();
    press("4");
    expect(behind).toHaveBeenCalledTimes(1);
  } finally {
    window.removeEventListener("keydown", behind, { capture: true });
  }
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
