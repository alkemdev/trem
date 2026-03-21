const PREVIEW_BOOT_FLAG = "__preview_bootstrap__";

const ROUTE_SEGMENTS = {
	terminal: "",
} as const;

export type AppRoute = keyof typeof ROUTE_SEGMENTS;

const splitPath = (pathname: string) => pathname.split("/").filter(Boolean);

const buildPath = (segments: string[]) => {
	if (segments.length === 0) {
		return "/";
	}
	return `/${segments.join("/")}`;
};

const isPreviewBootstrap = () => {
	const globals = window as unknown as Record<string, unknown>;
	return Boolean(globals[PREVIEW_BOOT_FLAG]);
};

export const getRouteContext = (pathname = window.location.pathname) => {
	const segments = splitPath(pathname);
	const previewBootstrap = isPreviewBootstrap();
	const baseSegments = previewBootstrap && segments.length > 0 ? [segments[0]] : [];
	const appSegments = previewBootstrap ? segments.slice(1) : segments;
	const routeSegment = appSegments[0]?.toLowerCase();
	const route: AppRoute = routeSegment === "terminal" ? "terminal" : "terminal";

	return {
		route,
		baseSegments,
		appSegments,
	};
};

export const getTerminalHref = (pathname = window.location.pathname) => {
	const { baseSegments } = getRouteContext(pathname);
	return buildPath(baseSegments);
};
