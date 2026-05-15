const test = require("node:test");
const assert = require("node:assert/strict");

const {
  DESKTOP_COMMANDS,
  DESKTOP_EVENTS,
} = require("../../.test-dist/src/protocol/desktopProtocol.js");
const {
  createValidationRunnerCapability,
} = require("../../.test-dist/src/runtime/capabilities/validationRunner.js");

test("desktop protocol names stay centralized and stable", () => {
  assert.deepEqual(DESKTOP_COMMANDS, {
    validate: "validate",
    cancelValidation: "cancel_validation",
    checkClanAvailable: "check_clan_available",
    openInClan: "open_in_clan",
    exportResults: "export_results",
  });
  assert.deepEqual(DESKTOP_EVENTS, {
    validation: "validation-event",
  });
});

test("validation runner listens before invoking and disposes once", async () => {
  const seenEvents = [];
  const invocations = [];
  let disposeCalls = 0;

  const transport = {
    async invoke(command, payload) {
      invocations.push([command, payload]);
      return undefined;
    },
    async listenValidationEvent(listener) {
      listener({ type: "discovering" });
      listener({ type: "started", totalFiles: 2 });
      return () => {
        disposeCalls += 1;
      };
    },
  };

  const validationRunner = createValidationRunnerCapability(transport);
  const run = await validationRunner.startValidation("/tmp/reference", (event) => {
    seenEvents.push(event);
  });

  assert.deepEqual(seenEvents, [
    { type: "discovering" },
    { type: "started", totalFiles: 2 },
  ]);
  assert.deepEqual(invocations, [
    [DESKTOP_COMMANDS.validate, { path: "/tmp/reference" }],
  ]);

  await run.cancel();
  assert.deepEqual(invocations[1], [DESKTOP_COMMANDS.cancelValidation, undefined]);

  run.dispose();
  run.dispose();
  assert.equal(disposeCalls, 1);
});

test("validation runner disposes the listener if validate fails", async () => {
  let disposed = false;

  const validationRunner = createValidationRunnerCapability({
    async invoke() {
      throw new Error("boom");
    },
    async listenValidationEvent() {
      return () => {
        disposed = true;
      };
    },
  });

  await assert.rejects(
    validationRunner.startValidation("/tmp/reference", () => {}),
    /boom/,
  );
  assert.equal(disposed, true);
});
