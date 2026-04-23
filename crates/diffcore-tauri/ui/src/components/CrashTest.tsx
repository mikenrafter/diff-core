import { useAppContext } from "../hooks/AppContext";

/** Test-only component that throws during render to exercise ErrorBoundary. */
export function CrashTest({ panel }: { panel: string }) {
  const { crashPanel } = useAppContext();
  if (crashPanel === panel) {
    throw new Error(`Test crash in ${panel}`);
  }
  return null;
}
