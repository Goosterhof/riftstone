import { afterEach, expect, it, vi } from "vitest";
import { useWorld } from "@/composables/useWorld";
import { useSearch } from "@/composables/useSearch";
import type { InstallPlanView, UninstallPlanView, InstallResultView } from "@/types/rift";
import { useToasts } from "@/composables/useToasts";
import { sleep, spine, typeQuery, until, wakeOn } from "./harness";

afterEach(() => {
  vi.restoreAllMocks();
  sleep();
});

it("does not reopen a closed option when its plan arrives", async () => {
  await wakeOn();
  await typeQuery("ermine");
  const first = useSearch().results.value[0];
  const world = useWorld();
  await world.open(first.optionId);
  const plan = await spine().planInstall(first.optionId);
  let release!: (value: InstallPlanView) => void;
  vi.spyOn(spine(), "planInstall").mockImplementation(
    () =>
      new Promise((resolve) => {
        release = resolve;
      }),
  );
  const pending = world.beginInstall();
  await until(() => !!release, "the plan requested");
  world.close();
  release(plan);
  await pending;
  expect(world.selectedId.value).toBeNull();
  expect(world.stage.value).toBe("detail");
  expect(world.planning.value).toBe(false);
});

it("does not reuse an old plan after leaving and reopening the same option", async () => {
  await wakeOn();
  await typeQuery("ermine");
  const first = useSearch().results.value[0];
  const world = useWorld();
  await world.open(first.optionId);
  const plan = await spine().planInstall(first.optionId);
  let release!: (value: InstallPlanView) => void;
  vi.spyOn(spine(), "planInstall").mockImplementation(
    () =>
      new Promise((resolve) => {
        release = resolve;
      }),
  );
  const pending = world.beginInstall();
  await until(() => !!release, "the first request");
  world.close();
  await world.open(first.optionId);
  release(plan);
  await pending;
  expect(world.stage.value).toBe("detail");
  expect(world.plan.value).toBeNull();
});

it("keeps a delayed uninstall preview with its original option", async () => {
  await wakeOn();
  await typeQuery("ermine");
  const [first, second] = useSearch().results.value;
  const world = useWorld();
  await world.open(first.optionId);
  let release!: (value: UninstallPlanView) => void;
  vi.spyOn(spine(), "planUninstall").mockImplementation(
    () =>
      new Promise((resolve) => {
        release = resolve;
      }),
  );
  const pending = world.beginUninstall();
  await until(() => !!release, "the uninstall preview");
  await world.open(second.optionId);
  release({ optionId: first.optionId, name: first.name, removals: [], refused: null });
  await pending;
  expect(world.detail.value?.option.id).toBe(second.optionId);
  expect(world.stage.value).toBe("detail");
  expect(world.returnPlan.value).toBeNull();
});

it("reports an install finishing in the background without changing the new selection", async () => {
  await wakeOn();
  await typeQuery("ermine");
  const [first, second] = useSearch().results.value;
  const world = useWorld();
  await world.open(first.optionId);
  await world.beginInstall();
  let release!: (value: InstallResultView) => void;
  const install = vi.spyOn(spine(), "install").mockImplementation(
    () =>
      new Promise((resolve) => {
        release = resolve;
      }),
  );
  const pending = world.confirmInstall();
  await until(() => install.mock.calls.length === 1, "the install started");
  await world.open(second.optionId);
  release({
    optionId: first.optionId,
    filesWritten: 1,
    bytesWritten: 1,
    backups: 0,
    durationMs: 1,
    restartNeeded: false,
    putAway: [],
    putAwayFailed: null,
  });
  await pending;
  expect(world.detail.value?.option.id).toBe(second.optionId);
  expect(world.stage.value).toBe("detail");
  expect(useToasts().toasts.value.some((t) => t.line.startsWith(`${first.name} is in your world.`))).toBe(true);
});

it("keeps a delayed install plan with the option that requested it", async () => {
  await wakeOn();
  await typeQuery("ermine");
  await until(() => useSearch().results.value.length >= 2, "two options");
  const [first, second] = useSearch().results.value;
  const world = useWorld();
  await world.open(first.optionId);
  const plan = await spine().planInstall(first.optionId);
  let release!: (value: InstallPlanView) => void;
  const delayed = vi.spyOn(spine(), "planInstall").mockImplementation(
    () =>
      new Promise((resolve) => {
        release = resolve;
      }),
  );
  const pending = world.beginInstall();
  await until(() => delayed.mock.calls.length === 1, "the first plan requested");
  await world.open(second.optionId);
  release(plan);
  await pending;
  expect(world.detail.value?.option.id).toBe(second.optionId);
  expect(world.stage.value).toBe("detail");
  expect(world.plan.value).toBeNull();
});
