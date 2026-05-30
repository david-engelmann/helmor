/**
 * React context that carries the active workspace's binding info to
 * inline-badge preview loaders. Without it, `createFilePreviewLoader`
 * falls back to the legacy local-only read — fine for surfaces with no
 * workspace context (rare), wrong for the composer / chat thread where
 * a workspace-bound remote runtime should serve the preview from the
 * container, not the laptop.
 *
 * The provider lives at the workspace-panel root so every composer
 * file-badge and chat-thread `@<path>` mention picks the same context
 * without each call site having to thread the workspaceRootPath +
 * workspaceId down by hand.
 */

import { createContext, type ReactNode, useContext, useMemo } from "react";

import type { FilePreviewContext } from "./preview-loader";

const Ctx = createContext<FilePreviewContext | null>(null);

export function FilePreviewProvider({
	workspaceRootPath,
	workspaceId,
	children,
}: {
	workspaceRootPath: string | null;
	workspaceId?: string | null;
	children: ReactNode;
}): ReactNode {
	const value = useMemo<FilePreviewContext | null>(
		() =>
			workspaceRootPath
				? { workspaceRootPath, workspaceId: workspaceId ?? undefined }
				: null,
		[workspaceRootPath, workspaceId],
	);
	return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

/** Returns the workspace context for inline-badge previews, or `null`
 *  when no workspace is bound at this point in the tree. */
export function useFilePreviewContext(): FilePreviewContext | null {
	return useContext(Ctx);
}
