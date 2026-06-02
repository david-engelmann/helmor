import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ScriptEvent, TerminalEventNotification } from "@/lib/api";

// ── Mocks ────────────────────────────────────────────────────────────────────
// Capture the event callback each `startScript` passes to `executeRepoScript`
// so tests can drive the stream synchronously without real IPC.

const apiMocks = vi.hoisted(() => ({
	executeRepoScript:
		vi.fn<
			(
				repoId: string,
				scriptType: "setup" | "run",
				onEvent: (event: ScriptEvent) => void,
				workspaceId?: string,
			) => Promise<void>
		>(),
	stopRepoScript: vi.fn(),
	writeRepoScriptStdin: vi.fn(),
	resizeRepoScript: vi.fn(),
	listWorkspaceRuntimeBindings: vi.fn(),
	openRemoteTerminal: vi.fn(),
	closeRemoteTerminal: vi.fn(),
	writeRemoteTerminal: vi.fn(),
	resizeRemoteTerminal: vi.fn(),
}));

vi.mock("@/lib/api", async (importOriginal) => {
	const actual = await importOriginal<typeof import("@/lib/api")>();
	return {
		...actual,
		executeRepoScript: apiMocks.executeRepoScript,
		stopRepoScript: apiMocks.stopRepoScript,
		writeRepoScriptStdin: apiMocks.writeRepoScriptStdin,
		resizeRepoScript: apiMocks.resizeRepoScript,
		listWorkspaceRuntimeBindings: apiMocks.listWorkspaceRuntimeBindings,
		openRemoteTerminal: apiMocks.openRemoteTerminal,
		closeRemoteTerminal: apiMocks.closeRemoteTerminal,
		writeRemoteTerminal: apiMocks.writeRemoteTerminal,
		resizeRemoteTerminal: apiMocks.resizeRemoteTerminal,
	};
});

// Dynamic import so vi.mock is applied before module evaluation.
const {
	_resetForTesting,
	getScriptState,
	resizeScript,
	startScript,
	stopScript,
	TRUNCATION_NOTICE,
	writeStdin,
} = await import("./script-store");

// ── Helpers ──────────────────────────────────────────────────────────────────

/** Start a script and return the event-injector bound to that run. */
function startAndCapture(workspaceId = "ws1") {
	let injector: ((event: ScriptEvent) => void) | null = null;
	apiMocks.executeRepoScript.mockImplementationOnce(
		async (_repoId, _scriptType, onEvent) => {
			injector = onEvent;
			// Return a pending promise; we drive `exited` manually.
			await new Promise(() => {});
		},
	);
	startScript("repo1", "run", workspaceId);
	if (!injector)
		throw new Error("executeRepoScript mock did not capture handler");
	return injector as (event: ScriptEvent) => void;
}

// ── Tests ────────────────────────────────────────────────────────────────────

const MAX_BYTES = 2 * 1024 * 1024;

beforeEach(() => {
	_resetForTesting();
	for (const m of Object.values(apiMocks)) m.mockReset();
	// Default: no bindings, so the remote-dispatch path degrades to
	// local. Per-test overrides set the binding when they need it.
	apiMocks.listWorkspaceRuntimeBindings.mockResolvedValue([]);
	apiMocks.openRemoteTerminal.mockResolvedValue({ pid: 12345 });
});

describe("script-store ring buffer", () => {
	it("keeps every chunk when total stays under the cap", () => {
		const emit = startAndCapture();
		emit({ type: "stdout", data: "hello\n" });
		emit({ type: "stderr", data: "warn\n" });

		const entry = getScriptState("ws1", "run");
		expect(entry).not.toBeNull();
		expect(entry?.chunks).toEqual(["hello\n", "warn\n"]);
		expect(entry?.bufferedBytes).toBe(11);
		expect(entry?.truncated).toBe(false);
	});

	it("evicts head chunks once total exceeds the byte cap", () => {
		const emit = startAndCapture();
		const chunk = "x".repeat(700_000); // 700 KB
		for (let i = 0; i < 5; i++) emit({ type: "stdout", data: chunk });

		const entry = getScriptState("ws1", "run");
		expect(entry?.truncated).toBe(true);
		// Never exceed the cap after stabilizing (every push eventually shrinks).
		expect(entry?.bufferedBytes).toBeLessThanOrEqual(MAX_BYTES);
		// bufferedBytes stays in sync with the remaining chunks.
		const actualSum = entry?.chunks.reduce((n, c) => n + c.length, 0);
		expect(actualSum).toBe(entry?.bufferedBytes);
	});

	it("keeps a single oversized chunk rather than dropping it entirely", () => {
		const emit = startAndCapture();
		const huge = "y".repeat(MAX_BYTES + 1024); // single chunk > cap
		emit({ type: "stdout", data: huge });

		const entry = getScriptState("ws1", "run");
		// length > 1 guard means a lone oversized chunk survives.
		expect(entry?.chunks.length).toBe(1);
		expect(entry?.truncated).toBe(false);
		expect(entry?.bufferedBytes).toBe(huge.length);
	});

	it("resets truncated/bufferedBytes on a fresh startScript for the same workspace", () => {
		const emit1 = startAndCapture("ws1");
		const chunk = "z".repeat(700_000);
		for (let i = 0; i < 5; i++) emit1({ type: "stdout", data: chunk });
		expect(getScriptState("ws1", "run")?.truncated).toBe(true);

		// Second run reuses the same key.
		startAndCapture("ws1");
		const entry = getScriptState("ws1", "run");
		expect(entry?.truncated).toBe(false);
		expect(entry?.bufferedBytes).toBe(0);
		expect(entry?.chunks).toEqual([]);
	});

	it("also trims chunks appended through the `error` event path", () => {
		const emit = startAndCapture();
		const chunk = "w".repeat(700_000);
		for (let i = 0; i < 4; i++) emit({ type: "stdout", data: chunk });
		emit({ type: "error", message: "boom" });

		const entry = getScriptState("ws1", "run");
		expect(entry?.truncated).toBe(true);
		expect(entry?.status).toBe("exited");
		// Error message is still the *last* chunk — tail is never evicted.
		expect(entry?.chunks[entry.chunks.length - 1]).toContain("boom");
	});

	it("exposes a truncation notice for replay prefixing", () => {
		expect(TRUNCATION_NOTICE).toContain("truncated");
		// ANSI dim + reset so we don't leak styling into replayed chunks.
		expect(TRUNCATION_NOTICE).toContain("\x1b[2m");
		expect(TRUNCATION_NOTICE).toContain("\x1b[0m");
	});
});

describe("script-store userStopped tracking", () => {
	it("fresh entries start with userStopped=false", () => {
		startAndCapture();
		expect(getScriptState("ws1", "run")?.userStopped).toBe(false);
	});

	it("stopScript marks the entry as user-initiated", () => {
		startAndCapture();
		stopScript("repo1", "run", "ws1");
		expect(getScriptState("ws1", "run")?.userStopped).toBe(true);
	});

	it("a subsequent startScript clears userStopped", () => {
		startAndCapture();
		stopScript("repo1", "run", "ws1");
		expect(getScriptState("ws1", "run")?.userStopped).toBe(true);

		startAndCapture();
		expect(getScriptState("ws1", "run")?.userStopped).toBe(false);
	});
});

describe("script-store dispatch (remote runtime)", () => {
	it("falls back to executeRepoScript when no binding exists", async () => {
		apiMocks.listWorkspaceRuntimeBindings.mockResolvedValueOnce([]);
		apiMocks.executeRepoScript.mockImplementationOnce(async () => {
			// no-op; just prove the local fallback fired.
		});

		startScript("repo-1", "setup", "ws-local", null, {
			command: "npm install",
			workspaceRootPath: "/laptop/path",
		});

		await vi.waitFor(() => {
			expect(apiMocks.executeRepoScript).toHaveBeenCalledTimes(1);
		});
		expect(apiMocks.openRemoteTerminal).not.toHaveBeenCalled();
		const entry = getScriptState("ws-local", "setup");
		expect(entry?.remoteRouting).toBeNull();
	});

	it("routes through openRemoteTerminal when bound to a non-local runtime", async () => {
		apiMocks.listWorkspaceRuntimeBindings.mockResolvedValueOnce([
			{
				workspaceId: "ws-remote",
				runtimeName: "docker-linux-arm64",
				remotePath: "/home/e2e/helmor-workspaces/helmor-taper",
			},
		]);
		let remoteCallback: ((e: TerminalEventNotification) => void) | null = null;
		apiMocks.openRemoteTerminal.mockImplementationOnce(
			async (_runtime, _termId, _ws, options) => {
				remoteCallback = options.onEvent;
				return { pid: 9999 };
			},
		);

		startScript("repo-1", "run", "ws-remote", "action-1", {
			command: "bun run dev",
			workspaceRootPath: "/laptop/path",
		});

		await vi.waitFor(() => {
			expect(apiMocks.openRemoteTerminal).toHaveBeenCalledTimes(1);
		});
		// remote_path beats the laptop path: the daem4on never sees a path
		// that only exists on the host.
		expect(apiMocks.openRemoteTerminal).toHaveBeenCalledWith(
			"docker-linux-arm64",
			expect.any(String),
			"/home/e2e/helmor-workspaces/helmor-taper",
			expect.objectContaining({ command: "bun run dev" }),
		);
		expect(apiMocks.executeRepoScript).not.toHaveBeenCalled();
		expect(remoteCallback).toBeTypeOf("function");

		await vi.waitFor(() => {
			const e = getScriptState("ws-remote", "run", "action-1");
			expect(e?.remoteRouting?.runtimeName).toBe("docker-linux-arm64");
		});
	});

	it("translates remote stdout / exited events into the local ScriptEvent shape", async () => {
		apiMocks.listWorkspaceRuntimeBindings.mockResolvedValueOnce([
			{ workspaceId: "ws-remote", runtimeName: "ssh.box", remotePath: "/code" },
		]);
		let remoteCallback: ((e: TerminalEventNotification) => void) | null = null;
		apiMocks.openRemoteTerminal.mockImplementationOnce(
			async (_r, _t, _w, options) => {
				remoteCallback = options.onEvent;
				return { pid: 1 };
			},
		);

		startScript("repo-1", "run", "ws-remote", "action-1", {
			command: "true",
			workspaceRootPath: "/code",
		});
		await vi.waitFor(() => expect(remoteCallback).toBeTypeOf("function"));
		const fire = remoteCallback as unknown as (
			e: TerminalEventNotification,
		) => void;
		fire({ terminalId: "x", event: { kind: "stdout", data: "hello\n" } });
		fire({ terminalId: "x", event: { kind: "exited", code: 0 } });

		const entry = getScriptState("ws-remote", "run", "action-1");
		expect(entry?.chunks.join("")).toBe("hello\n");
		expect(entry?.status).toBe("exited");
		expect(entry?.exitCode).toBe(0);
	});

	it("falls through to workspaceRootPath when the binding has no remotePath", async () => {
		apiMocks.listWorkspaceRuntimeBindings.mockResolvedValueOnce([
			{ workspaceId: "ws-remote", runtimeName: "ssh.box", remotePath: null },
		]);

		startScript("repo-1", "setup", "ws-remote", null, {
			command: "npm install",
			workspaceRootPath: "/code/foo",
		});

		await vi.waitFor(() => {
			expect(apiMocks.openRemoteTerminal).toHaveBeenCalledTimes(1);
		});
		expect(apiMocks.openRemoteTerminal).toHaveBeenCalledWith(
			"ssh.box",
			expect.any(String),
			"/code/foo",
			expect.any(Object),
		);
	});

	it("treats a binding to the literal `local` runtime as local", async () => {
		apiMocks.listWorkspaceRuntimeBindings.mockResolvedValueOnce([
			{ workspaceId: "ws-local", runtimeName: "local", remotePath: null },
		]);
		startScript("repo-1", "setup", "ws-local", null, {
			command: "npm install",
			workspaceRootPath: "/code/foo",
		});
		await vi.waitFor(() => {
			expect(apiMocks.executeRepoScript).toHaveBeenCalledTimes(1);
		});
		expect(apiMocks.openRemoteTerminal).not.toHaveBeenCalled();
	});

	it("dispatches stop/write/resize through the remote RPCs for a remote-bound entry", async () => {
		apiMocks.listWorkspaceRuntimeBindings.mockResolvedValueOnce([
			{ workspaceId: "ws-remote", runtimeName: "ssh.box", remotePath: "/code" },
		]);
		let capturedTerminalId: string | null = null;
		apiMocks.openRemoteTerminal.mockImplementationOnce(
			async (_runtime, terminalId, _w, _options) => {
				capturedTerminalId = terminalId;
				return { pid: 1 };
			},
		);
		startScript("repo-1", "run", "ws-remote", "action-1", {
			command: "bun run dev",
			workspaceRootPath: "/code",
		});
		await vi.waitFor(() => {
			expect(
				getScriptState("ws-remote", "run", "action-1")?.remoteRouting,
			).not.toBeNull();
		});

		writeStdin("repo-1", "run", "ws-remote", "data", "action-1");
		expect(apiMocks.writeRemoteTerminal).toHaveBeenCalledWith(
			"ssh.box",
			capturedTerminalId,
			"data",
		);
		expect(apiMocks.writeRepoScriptStdin).not.toHaveBeenCalled();

		resizeScript("repo-1", "run", "ws-remote", 100, 30, "action-1");
		expect(apiMocks.resizeRemoteTerminal).toHaveBeenCalledWith(
			"ssh.box",
			capturedTerminalId,
			100,
			30,
		);
		expect(apiMocks.resizeRepoScript).not.toHaveBeenCalled();

		stopScript("repo-1", "run", "ws-remote", "action-1");
		expect(apiMocks.closeRemoteTerminal).toHaveBeenCalledWith(
			"ssh.box",
			capturedTerminalId,
		);
		expect(apiMocks.stopRepoScript).not.toHaveBeenCalled();
	});

	it("degrades to local when the binding lookup throws", async () => {
		apiMocks.listWorkspaceRuntimeBindings.mockRejectedValueOnce(
			new Error("disk read failed"),
		);
		startScript("repo-1", "setup", "ws-fallback", null, {
			command: "npm install",
			workspaceRootPath: "/code",
		});
		await vi.waitFor(() => {
			expect(apiMocks.executeRepoScript).toHaveBeenCalledTimes(1);
		});
		expect(apiMocks.openRemoteTerminal).not.toHaveBeenCalled();
	});
});
