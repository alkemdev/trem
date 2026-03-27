import { type ComponentType, useEffect, useState } from "react";
import TerminalApp from "./TerminalApp";
import { AppRoute, getRouteContext } from "./routing";

const resolveRoute = (): AppRoute => getRouteContext().route;
const ROUTE_COMPONENTS: Record<AppRoute, ComponentType> = {
	terminal: TerminalApp,
};

function App() {
	const [route, setRoute] = useState(resolveRoute);

	useEffect(() => {
		const handleRoute = () => {
			setRoute(resolveRoute());
		};

		window.addEventListener("popstate", handleRoute);
		window.addEventListener("hashchange", handleRoute);

		return () => {
			window.removeEventListener("popstate", handleRoute);
			window.removeEventListener("hashchange", handleRoute);
		};
	}, []);

	const RouteComponent = ROUTE_COMPONENTS[route];
	return <RouteComponent />;
}

export default App;
