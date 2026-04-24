/**
 * SourceTab — right-panel source explorer for navigating symbols in the diff.
 *
 * Renders a SourceExplorer that allows jumping to symbols and scrolling the
 * DiffViewer to specific lines. All state is consumed from AppContext.
 */
import SourceExplorer from "../SourceExplorer";
import { useAppContext } from "../../hooks/AppContext";

export function SourceTab() {
  const {
    fileDiff,
    selectedGroup,
    selectedFileChange,
    sourceFocusRequest,
    handleSourceNavigate,
    diffViewerRef,
  } = useAppContext();

  return (
    <SourceExplorer
      fileDiff={fileDiff}
      selectedGroup={selectedGroup}
      selectedFileChange={selectedFileChange}
      focusRequest={sourceFocusRequest}
      onNavigateToSymbol={handleSourceNavigate}
      onScrollToLine={(startLine, endLine) => {
        diffViewerRef.current?.scrollToLine(startLine, endLine);
      }}
    />
  );
}
