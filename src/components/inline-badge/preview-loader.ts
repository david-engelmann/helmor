/**
 * Lazy preview loader for file-based `InlineBadge`s.
 *
 * Call `createFilePreviewLoader(path, ctx?)` to obtain a zero-arg function that,
 * when invoked, returns a promise resolving to a `ComposerPreviewPayload`.
 * The payload is cached at the module level keyed by path, so repeated
 * hovers of the same file (across multiple badges) only trigger a single
 * read.
 *
 * When `ctx` carries `workspaceRootPath` (and optionally `workspaceId`), the
 * read goes through the binding-aware seam — a workspace pinned to a remote
 * runtime preview-loads from the remote container, not the laptop. Without
 * `ctx`, the loader falls back to the absolute-path local read; useful for
 * call sites with no workspace context.
 *
 * The loader throws for unreadable files (binary, missing, permission
 * denied). Callers should render a "Unable to preview" frame on rejection.
 * For files that exceed `MAX_PREVIEW_BYTES`, the loader resolves with a
 * text payload containing a "too large" hint instead of reading.
 */

import {
	readEditorFile,
	readWorkspaceFile,
	statEditorFile,
	statWorkspaceFile,
	toWorkspaceRelativePath,
} from "@/lib/api";
import {
	type ComposerPreviewPayload,
	inferComposerPreviewLanguage,
} from "@/lib/composer-insert";
import { basename } from "@/lib/path-util";

/** Max file size (in bytes) we attempt to read for a preview. */
const MAX_PREVIEW_BYTES = 512 * 1024; // 512 KB

export type FilePreviewContext = {
	workspaceRootPath: string;
	workspaceId?: string;
};

/** Module-level cache: cacheKey -> in-flight or settled promise. */
const previewCache = new Map<string, Promise<ComposerPreviewPayload>>();

/** Clear the in-memory preview cache. Useful in tests. */
export function clearInlineBadgePreviewCache(): void {
	previewCache.clear();
}

export function createFilePreviewLoader(
	path: string,
	ctx?: FilePreviewContext,
): () => Promise<ComposerPreviewPayload> {
	// The cache key folds in the workspace context so a path that resolves
	// to different files across two runtimes (e.g. `/repo/README.md` on
	// laptop vs. on the remote container) can't return stale content from
	// the wrong machine.
	const cacheKey = ctx
		? `${ctx.workspaceRootPath}|${ctx.workspaceId ?? ""}|${path}`
		: `local|${path}`;
	return () => {
		const existing = previewCache.get(cacheKey);
		if (existing) return existing;

		const pending = loadFilePreview(path, ctx);
		previewCache.set(cacheKey, pending);

		// On failure, evict so the next hover gets a retry.
		pending.catch(() => {
			previewCache.delete(cacheKey);
		});

		return pending;
	};
}

async function loadFilePreview(
	path: string,
	ctx: FilePreviewContext | undefined,
): Promise<ComposerPreviewPayload> {
	const title = basename(path);

	// Stat first so we can short-circuit huge files without loading them.
	const stat = ctx
		? await statWorkspaceFile(
				ctx.workspaceRootPath,
				toWorkspaceRelativePath(ctx.workspaceRootPath, path),
				ctx.workspaceId,
			)
		: await statEditorFile(path);
	if (!stat.exists || !stat.isFile) {
		throw new Error(`File not found or not a regular file: ${path}`);
	}
	if (stat.size !== null && stat.size > MAX_PREVIEW_BYTES) {
		const mb = (stat.size / (1024 * 1024)).toFixed(1);
		return {
			kind: "text",
			title,
			text: `File too large to preview (${mb} MB)`,
		};
	}

	// The reader throws for binary / non-UTF-8 content.
	const response = ctx
		? await readWorkspaceFile(
				ctx.workspaceRootPath,
				toWorkspaceRelativePath(ctx.workspaceRootPath, path),
				ctx.workspaceId,
			)
		: await readEditorFile(path);
	const language = inferComposerPreviewLanguage(response.content);

	if (language) {
		return {
			kind: "code",
			title,
			code: response.content,
			language,
		};
	}

	return {
		kind: "text",
		title,
		text: response.content,
	};
}
