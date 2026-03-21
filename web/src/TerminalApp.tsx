import "./App.css";
import { useEffect, useRef, useState } from "react";
import initWasm, { start_trem_web } from "../pkg/trem_web";

function TerminalApp() {
	const terminalRef = useRef<HTMLDivElement>(null);
	const startedRef = useRef(false);
	const [error, setError] = useState<string | null>(null);

	useEffect(() => {
		if (startedRef.current) {
			return;
		}
		startedRef.current = true;

		void initWasm()
			.then(() => {
				if (!terminalRef.current) {
					throw new Error("terminal container was not mounted");
				}
				start_trem_web("trem-terminal");
			})
			.catch((err: unknown) => {
				const message = err instanceof Error ? err.message : String(err);
				setError(message);
				console.error("Failed to start trem web TUI", err);
			});
	}, []);

	if (error) {
		return (
			<div
				style={{
					height: "100%",
					width: "100%",
					display: "flex",
					alignItems: "center",
					justifyContent: "center",
					padding: "1rem",
				}}
			>
				<pre style={{ color: "#ff7b72", whiteSpace: "pre-wrap" }}>
					{`Failed to start Trem web TUI:\n${error}`}
				</pre>
			</div>
		);
	}

	return (
		<div
			id="trem-terminal"
			ref={terminalRef}
			style={{ height: "100%", width: "100%", textAlign: "left" }}
		></div>
	);
}

export default TerminalApp;
