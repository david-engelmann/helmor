import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ScriptEvent, TerminalEventNotification } from "@/lib/api";

/**
 * Tests the binding-aware dispatch in `terminal-store.ts`. The store has
 * one observable behaviour per routing axis:
 *
 *  - workspace with NO binding → `spawnTerminal` (local PTY); writes /
 *    resizes / closes go through the local Tauri commands.
 *  - workspace bound to a NON-`local` runtime with a `remotePath` set
 *    → `openRemoteTerminal` with the override; writes / resizes / closes
 *    go through the remote Tauri commands and never touch the local
 *    PTY surface.
 *  - workspace bound to `local` (the literal runtime key) → still local.
 *  - binding lookup blows up → degrade to local rather than refuse to
 *    open a shell.
 *
 * Each test drives the dispatch synchronously by capturing the
 * callback the store hands to whichever spawn it picks, then asserting
 * that the write/resize/close functions invoked the matching backend
 * (and only that one).
 */

const apiMocks = vi.hoisted(() => ({
	listWorkspaceRuntimeBindings: vi.fn(),
	spawnTerminal: vi.fn(),
	stopTerminal: vi.fn(),
	writeTerminalStdin: vi.fn(),
	resizeTerminal: vi.fn(),
	openRemoteTerminal: vi.fn(),
	closeRemoteTerminal: vi.fn(),
	writeRemoteTerminal: vi.fn(),
	resizeRemoteTerminal: vi.fn(),
}));

vi.mock("@/lib/api", async (importOriginal) => {
	const actual = await importOriginal<typeof import("@/lib/api")>();
	return {
		...actual,
		listWorkspaceRuntimeBindings: apiMocks.listWorkspaceRuntimeBindings,
		spawnTerminal: apiMocks.spawnTerminal,
		stopTerminal: apiMocks.stopTerminal,
		writeTerminalStdin: apiMocks.writeTerminalStdin,
		resizeTerminal: apiMocks.resizeTerminal,
		openRemoteTerminal: apiMocks.openRemoteTerminal,
		closeRemoteTerminal: apiMocks.closeRemoteTerminal,
		writeRemoteTerminal: apiMocks.writeRemoteTerminal,
		resizeRemoteTerminal: apiMocks.resizeRemoteTerminal,
	};
});

const { closeTerminal, createTerminal, getTerminals, resize, writeStdin } =
	await import("./terminal-store");

function resetState(): void {
	// Drain whatever leftover instances the previous test left in the
	// module-level map FIRST. `closeTerminal` fires the local-or-remote
	// teardown spy on the way out — so do this before the mockReset so
	// the resulting calls are wiped along with everything else.
	for (const wsId of ["ws-local", "ws-remote", "ws-fallback"]) {
		for (const t of getTerminals(wsId)) {
			closeTerminal("repo-x", wsId, t.id);
		}
	}
	for (const m of Object.values(apiMocks)) m.mockReset();
	// Defaults: most tests just want "no bindings persisted".
	apiMocks.listWorkspaceRuntimeBindings.mockResolvedValue([]);
	apiMocks.spawnTerminal.mockResolvedValue(undefined);
	apiMocks.openRemoteTerminal.mockResolvedValue({ pid: 12345 });
}

beforeEach(resetState);

describe("terminal-store dispatch", () => {
	it("falls back to spawnTerminal when the workspace has no binding", async () => {
		// Capture the spawn callback so we can prove it's wired in.
		let localCallback: ((event: ScriptEvent) => void) | null = null;
		apiMocks.spawnTerminal.mockImplementationOnce(
			async (_repo, _ws, _id, onEvent) => {
				localCallback = onEvent;
			},
		);

		const instance = createTerminal("repo-1", "ws-local", "/local/path");
		expect(instance).not.toBeNull();
		if (!instance) return;

		// Routing is async; flush the microtask queue + the lookup promise.
		await vi.waitFor(() => {
			expect(apiMocks.spawnTerminal).toHaveBeenCalledTimes(1);
		});
		expect(apiMocks.openRemoteTerminal).not.toHaveBeenCalled();
		expect(localCallback).toBeTypeOf("function");

		// runtimeName must stay null so write/resize/close stay on the local path.
		const list = getTerminals("ws-local");
		expect(list[0]?.runtimeName).toBeNull();

		writeStdin("repo-1", "ws-local", instance.id, "echo hi\n");
		expect(apiMocks.writeTerminalStdin).toHaveBeenCalledWith(
			"repo-1",
			"ws-local",
			instance.id,
			"echo hi\n",
		);
		expect(apiMocks.writeRemoteTerminal).not.toHaveBeenCalled();

		resize("repo-1", "ws-local", instance.id, 100, 30);
		expect(apiMocks.resizeTerminal).toHaveBeenCalledWith(
			"repo-1",
			"ws-local",
			instance.id,
			100,
			30,
		);
		expect(apiMocks.resizeRemoteTerminal).not.toHaveBeenCalled();
	});

	it("opens a remote terminal when the workspace is bound to a non-local runtime", async () => {
		apiMocks.listWorkspaceRuntimeBindings.mockResolvedValueOnce([
			{
				workspaceId: "ws-remote",
				runtimeName: "docker-linux-arm64",
				remotePath: "/home/e2e/helmor-workspaces/helmor-taper",
			},
		]);

		let remoteCallback: ((event: TerminalEventNotification) => void) | null =
			null;
		apiMocks.openRemoteTerminal.mockImplementationOnce(
			async (_runtime, _termId, _ws, options) => {
				remoteCallback = options.onEvent;
				return { pid: 9999 };
			},
		);

		const instance = createTerminal(
			"repo-1",
			"ws-remote",
			"/Users/david/laptop/path",
		);
		expect(instance).not.toBeNull();
		if (!instance) return;

		await vi.waitFor(() => {
			expect(apiMocks.openRemoteTerminal).toHaveBeenCalledTimes(1);
		});
		// The remote_path override beats the laptop path: a remote PTY
		// must never receive a path that only exists on the host.
		expect(apiMocks.openRemoteTerminal).toHaveBeenCalledWith(
			"docker-linux-arm64",
			instance.id,
			"/home/e2e/helmor-workspaces/helmor-taper",
			expect.objectContaining({
				cols: expect.any(Number),
				rows: expect.any(Number),
			}),
		);
		expect(apiMocks.spawnTerminal).not.toHaveBeenCalled();
		expect(remoteCallback).toBeTypeOf("function");

		// runtimeName must be filled in so subsequent write/resize/close
		// route remote, not local.
		await vi.waitFor(() => {
			const list = getTerminals("ws-remote");
			expect(list[0]?.runtimeName).toBe("docker-linux-arm64");
		});

		writeStdin("repo-1", "ws-remote", instance.id, "ls\n");
		expect(apiMocks.writeRemoteTerminal).toHaveBeenCalledWith(
			"docker-linux-arm64",
			instance.id,
			"ls\n",
		);
		expect(apiMocks.writeTerminalStdin).not.toHaveBeenCalled();

		resize("repo-1", "ws-remote", instance.id, 120, 40);
		expect(apiMocks.resizeRemoteTerminal).toHaveBeenCalledWith(
			"docker-linux-arm64",
			instance.id,
			120,
			40,
		);
		expect(apiMocks.resizeTerminal).not.toHaveBeenCalled();

		closeTerminal("repo-1", "ws-remote", instance.id);
		expect(apiMocks.closeRemoteTerminal).toHaveBeenCalledWith(
			"docker-linux-arm64",
			instance.id,
		);
		expect(apiMocks.stopTerminal).not.toHaveBeenCalled();
	});

	it("falls through to the workspaceRootPath when remotePath is null", async () => {
		// Same-path setup (macOS and Linux both at /Users/dev/code/foo,
		// for example). The store should pass the local rootPath as the
		// remote workspace_dir.
		apiMocks.listWorkspaceRuntimeBindings.mockResolvedValueOnce([
			{
				workspaceId: "ws-remote",
				runtimeName: "ssh.dev.box",
				remotePath: null,
			},
		]);

		const instance = createTerminal("repo-1", "ws-remote", "/code/foo");
		if (!instance) throw new Error("createTerminal returned null");

		await vi.waitFor(() => {
			expect(apiMocks.openRemoteTerminal).toHaveBeenCalledTimes(1);
		});
		expect(apiMocks.openRemoteTerminal).toHaveBeenCalledWith(
			"ssh.dev.box",
			instance.id,
			"/code/foo",
			expect.any(Object),
		);
	});

	it("treats a binding to the `local` runtime as local", async () => {
		apiMocks.listWorkspaceRuntimeBindings.mockResolvedValueOnce([
			{ workspaceId: "ws-local", runtimeName: "local", remotePath: null },
		]);

		createTerminal("repo-1", "ws-local", "/code/foo");

		await vi.waitFor(() => {
			expect(apiMocks.spawnTerminal).toHaveBeenCalledTimes(1);
		});
		expect(apiMocks.openRemoteTerminal).not.toHaveBeenCalled();
	});

	it("degrades to local when the binding lookup fails", async () => {
		// A flaky disk read or a deserialisation failure should never
		// stop the user from opening a shell — keep going on the local
		// runtime rather than refusing the action.
		apiMocks.listWorkspaceRuntimeBindings.mockRejectedValueOnce(
			new Error("disk read failed"),
		);

		createTerminal("repo-1", "ws-fallback", "/code/foo");

		await vi.waitFor(() => {
			expect(apiMocks.spawnTerminal).toHaveBeenCalledTimes(1);
		});
		expect(apiMocks.openRemoteTerminal).not.toHaveBeenCalled();
	});

	it("buffers stdout from the remote stream into the instance's chunks", async () => {
		apiMocks.listWorkspaceRuntimeBindings.mockResolvedValueOnce([
			{
				workspaceId: "ws-remote",
				runtimeName: "docker-linux-arm64",
				remotePath: "/remote/path",
			},
		]);

		let remoteCallback: ((event: TerminalEventNotification) => void) | null =
			null;
		apiMocks.openRemoteTerminal.mockImplementationOnce(
			async (_runtime, _termId, _ws, options) => {
				remoteCallback = options.onEvent;
				return { pid: 1 };
			},
		);

		const instance = createTerminal("repo-1", "ws-remote", "/remote/path");
		if (!instance) throw new Error("createTerminal returned null");
		await vi.waitFor(() => expect(remoteCallback).toBeTypeOf("function"));
		const fire = remoteCallback as unknown as (
			e: TerminalEventNotification,
		) => void;

		// Push the remote shape; the adapter should translate it into
		// the same chunk-buffer the local path uses.
		fire({
			terminalId: instance.id,
			event: { kind: "stdout", data: "$ pwd\r\n/remote/path\r\n" },
		});
		const list = getTerminals("ws-remote");
		expect(list[0]?.chunks.join("")).toBe("$ pwd\r\n/remote/path\r\n");

		fire({
			terminalId: instance.id,
			event: { kind: "exited", code: 0 },
		});
		await vi.waitFor(() => {
			const updated = getTerminals("ws-remote");
			expect(updated[0]?.status).toBe("exited");
			expect(updated[0]?.exitCode).toBe(0);
		});
	});
});
