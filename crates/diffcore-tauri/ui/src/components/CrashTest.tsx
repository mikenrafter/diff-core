import { useAppContext } from "../hooks/AppContext";

/**
 * Test-only component that throws during render to exercise React ErrorBoundary.
 * Set `crashPanel` in context to the panel name to trigger a crash in that panel.
 */
export function CrashTest({ panel }: { panel: string }) {
  const { crashPanel } = useAppContext();
  if (crashPanel === panel) {
    throw new Error(`Test crash in ${panel}`);
  }
  return null;
}
