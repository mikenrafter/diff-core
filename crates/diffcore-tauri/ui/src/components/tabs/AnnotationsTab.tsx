/**
 * AnnotationsTab — right-panel annotations view for a selected flow group.
 *
 * Shows three sub-tabs:
 *  - Info: group metadata, LLM summary, deep analysis, file annotations
 *  - Graph: interactive flow graph with granularity control
 *  - Edges: tabular edge list with clickable endpoints
 *
 * Falls back to an empty-state prompt when no group is selected. All state
 * is consumed from AppContext — no props required.
 */
import type { LlmProvider } from "../../types";
import Dropdown from "../Dropdown";
import FlowGraph from "../FlowGraph";
import FileDisplay from "../FileDisplay";
import ErrorBoundary from "../ErrorBoundary";
import { shortPath, shortSymbol, symbolFilePath } from "../../utils/pathUtils";
import { PROVIDER_LABELS } from "../../utils/llmUtils";
import { useAppContext } from "../../hooks/AppContext";

/**
 * Throws during render when `crashPanel` matches, exercising ErrorBoundary.
 * Only active in tests via the `setCrashPanel` context setter.
 */
function CrashTest({ panel }: { panel: string }) {
  const { crashPanel } = useAppContext();
  if (crashPanel === panel) {
    throw new Error(`Test crash in ${panel}`);
  }
  return null;
}

export function AnnotationsTab() {
  const {
    selectedGroup,
    annotationSubTab,
    setAnnotationSubTab,
    groupAnnotation,
    groupDeepAnalysis,
    overview,
    refinementVerdict,
    llmSettings,
    replayActive,
    replayStep,
    enterReplay,
    exitReplay,
    modelsForProvider,
    updateSetting,
    setRegenDialogOpen,
    setRegenFeedbackText,
    setRegenIncludePreviousOutput,
    handleGraphNodeClick,
    handleGraphEdgeClick,
    handleEdgeEndpointClick,
    graphGranularity,
    setGraphGranularity,
  } = useAppContext();

  if (!selectedGroup) {
    return (
      <div className="empty-state">
        Select a group to see annotations.
      </div>
    );
  }

  return (
    <div className="annotations-scroll-container">
      {/* Annotation sub-tabs */}
      <div className="annotation-subtabs">
        <button
          className={`annotation-subtab ${annotationSubTab === "info" ? "active" : ""}`}
          onClick={() => setAnnotationSubTab("info")}
        >
          Info
        </button>
        <button
          className={`annotation-subtab ${annotationSubTab === "graph" ? "active" : ""}`}
          onClick={() => setAnnotationSubTab("graph")}
          disabled={selectedGroup.edges.length === 0}
        >
          Graph
        </button>
        <button
          className={`annotation-subtab ${annotationSubTab === "edges" ? "active" : ""}`}
          onClick={() => setAnnotationSubTab("edges")}
          disabled={selectedGroup.edges.length === 0}
        >
          Edges
          {selectedGroup.edges.length > 0 && (
            <span className="annotation-subtab-count">{selectedGroup.edges.length}</span>
          )}
        </button>
      </div>

      {annotationSubTab === "info" && (
        <>
          <div className="annotation-section" data-testid="annotations-panel">
            <h3>Flow Group</h3>
            <p className="group-detail-name">{selectedGroup.name}</p>
            {selectedGroup.entrypoint && (
              <p className="entrypoint-info">
                Entrypoint: {selectedGroup.entrypoint.symbol} (
                {selectedGroup.entrypoint.entrypoint_type})
              </p>
            )}
            <p>
              Risk: <strong>{selectedGroup.risk_score.toFixed(2)}</strong>{" "}
              | Files: <strong>{selectedGroup.files.length}</strong> |
              Review order: <strong>#{selectedGroup.review_order}</strong>
            </p>
            {llmSettings && (
              <>
                <div className="settings-row" style={{ marginTop: 8, alignItems: "center" }}>
                  <label style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                    Model ({PROVIDER_LABELS[llmSettings.provider as LlmProvider]})
                  </label>
                  <div style={{ marginLeft: "auto", maxWidth: 260, flex: "0 1 260px" }}>
                    <Dropdown
                      value={llmSettings.model}
                      onChange={(value) => updateSetting("model", value)}
                      options={modelsForProvider(llmSettings.provider).map((m) => ({ value: m, label: m }))}
                      placeholder="Select model"
                    />
                  </div>
                </div>
                <button
                  className="btn"
                  style={{ marginTop: 8 }}
                  onClick={() => {
                    setRegenDialogOpen(true);
                    setRegenFeedbackText("");
                    setRegenIncludePreviousOutput(true);
                  }}
                  title="Regenerate annotations with additional guidance"
                >
                  Regenerate with feedback/question
                </button>
              </>
            )}
            {selectedGroup.files.length > 1 && !replayActive && (
              <button
                className="btn btn-replay"
                onClick={enterReplay}
                title="Step through files in data flow order (r)"
              >
                &#9654; Replay Flow
              </button>
            )}
            {replayActive && (
              <button
                className="btn btn-replay-exit"
                onClick={exitReplay}
                title="Exit replay mode (Esc)"
              >
                &#10005; Exit Replay
              </button>
            )}
          </div>

          {refinementVerdict && (
            <div className="annotation-section refinement-verdict-section" data-testid="refinement-verdict">
              <h3>Refinement Verdict</h3>
              <p className="refinement-verdict-title">{refinementVerdict.title}</p>
              <p className="refinement-verdict-meta">
                {PROVIDER_LABELS[refinementVerdict.provider as LlmProvider] ?? refinementVerdict.provider}/{refinementVerdict.model}
              </p>
              {refinementVerdict.reasoning && (
                <p className="refinement-verdict-reasoning">{refinementVerdict.reasoning}</p>
              )}
            </div>
          )}

          {overview && !groupAnnotation && (
            <div className="annotation-section llm-section">
              <h3>LLM Overview</h3>
              <p className="llm-summary">{overview.overall_summary}</p>
            </div>
          )}

          {groupAnnotation && (
            <div className="annotation-section llm-section">
              <h3>LLM Summary</h3>
              <p className="llm-summary">{groupAnnotation.summary}</p>
              <p className="llm-rationale">
                <strong>Review rationale:</strong> {groupAnnotation.review_order_rationale}
              </p>
              {groupAnnotation.risk_flags.length > 0 && (
                <div className="risk-flags">
                  {groupAnnotation.risk_flags.map((flag, i) => (
                    <span key={i} className="risk-flag">{flag}</span>
                  ))}
                </div>
              )}
            </div>
          )}

          {overview && groupAnnotation && (
            <div className="annotation-section llm-section">
              <h3>Overall Summary</h3>
              <p className="llm-summary">{overview.overall_summary}</p>
            </div>
          )}

          {groupDeepAnalysis && (
            <>
              <div className="annotation-section llm-section">
                <h3>Flow Narrative</h3>
                <p className="llm-narrative">{groupDeepAnalysis.flow_narrative}</p>
              </div>

              {groupDeepAnalysis.file_annotations.length > 0 && (
                <div className="annotation-section llm-section">
                  <h3>File Annotations</h3>
                  {groupDeepAnalysis.file_annotations.map((fa, i) => (
                    <div key={i} className="file-annotation">
                      <div className="file-annotation-header">
                        <span className="file-annotation-path">{shortPath(fa.file)}</span>
                        <span className="file-annotation-role">{fa.role_in_flow}</span>
                      </div>
                      <p className="file-annotation-changes">{fa.changes_summary}</p>
                      {fa.risks.length > 0 && (
                        <div className="file-annotation-list">
                          <span className="annotation-label risk-label">Risks:</span>
                          <ul>
                            {fa.risks.map((r, j) => (
                              <li key={j}>{r}</li>
                            ))}
                          </ul>
                        </div>
                      )}
                      {fa.suggestions.length > 0 && (
                        <div className="file-annotation-list">
                          <span className="annotation-label suggestion-label">Suggestions:</span>
                          <ul>
                            {fa.suggestions.map((s, j) => (
                              <li key={j}>{s}</li>
                            ))}
                          </ul>
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              )}

              {groupDeepAnalysis.cross_cutting_concerns.length > 0 && (
                <div className="annotation-section llm-section">
                  <h3>Cross-Cutting Concerns</h3>
                  <ul className="concerns-list">
                    {groupDeepAnalysis.cross_cutting_concerns.map((c, i) => (
                      <li key={i}>{c}</li>
                    ))}
                  </ul>
                </div>
              )}
            </>
          )}
        </>
      )}

      {annotationSubTab === "graph" && selectedGroup.edges.length > 0 && (
        <div className="annotation-section flow-graph-section flow-graph-full">
          <div className="settings-row" style={{ marginBottom: 8, alignItems: "center" }}>
            <label style={{ fontSize: 12, color: "var(--text-secondary)" }}>Granularity</label>
            <div style={{ marginLeft: "auto", maxWidth: 220, flex: "0 1 220px" }}>
              <Dropdown<"file" | "module_class_method">
                value={graphGranularity}
                onChange={(value) => setGraphGranularity(value)}
                options={[
                  { value: "file", label: "file" },
                  { value: "module_class_method", label: "module/class/method", description: "preview" },
                ]}
              />
            </div>
          </div>
          {graphGranularity === "module_class_method" && (
            <p className="settings-hint" style={{ marginBottom: 8 }}>
              Preview mode: symbol-level graph is not available yet; rendering file-level graph as fallback.
            </p>
          )}
          <ErrorBoundary panelName="Flow Graph">
            <CrashTest panel="Flow Graph" />
            <FlowGraph
              edges={selectedGroup.edges}
              files={selectedGroup.files}
              onNodeClick={handleGraphNodeClick}
              onEdgeClick={handleGraphEdgeClick}
              replayNodeId={replayActive && selectedGroup.files[replayStep] ? selectedGroup.files[replayStep].path : null}
            />
          </ErrorBoundary>
        </div>
      )}

      {annotationSubTab === "edges" && selectedGroup.edges.length > 0 && (
        <div className="annotation-section edges-section">
          <ul className="edge-list">
            {selectedGroup.edges.map((edge, i) => {
              const fromFile = symbolFilePath(edge.from);
              const fromSymbol = shortSymbol(edge.from);
              const toLabel = shortSymbol(edge.to);
              return (
                <li key={i} className="edge-item file-item edge-item-row">
                  <FileDisplay
                    path={fromFile}
                    roleBadge={edge.edge_type}
                    hideChanges
                    prefix={(
                      <span className="edge-target">
                        <span className="edge-from-symbol" title={edge.from}>
                          {fromSymbol}
                        </span>
                        <span className="edge-arrow" aria-hidden="true">&rarr;</span>
                        <button
                          className="edge-endpoint edge-to"
                          onClick={() => handleEdgeEndpointClick(edge.to)}
                          title={edge.to}
                        >
                          {toLabel}
                        </button>
                      </span>
                    )}
                  />
                </li>
              );
            })}
          </ul>
        </div>
      )}
    </div>
  );
}
