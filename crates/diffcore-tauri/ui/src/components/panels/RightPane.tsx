import { useAppContext } from "../../hooks/AppContext";
import { ActivityTab } from "../tabs/ActivityTab";
import { AnnotationsTab } from "../tabs/AnnotationsTab";
import { SourceTab } from "../tabs/SourceTab";
import { CommentsTab } from "../tabs/CommentsTab";
import { PROVIDER_LABELS } from "../../utils/llmUtils";

/**
 * Right panel — tabbed view for LLM activity, comments, annotations, and
 * source explorer. Also renders the drag handle that resizes this panel.
 */
export function RightPane() {
  const {
    rightPanelCollapsed, setRightPanelCollapsed,
    rightPanelTab, setRightPanelTab,
    rightPanelWidth,
    activityJob, activityTimeline,
    comments,
    annotating, deepAnalyzing, refining,
    aiAccessReady, llmSettings, openAiSetup,
  } = useAppContext();

  return (
    <>
        {/* Right panel: Activity + Annotations */}
        <aside
          className={`panel panel-right ${rightPanelCollapsed ? "panel-right-collapsed" : ""}`}
          style={rightPanelCollapsed ? undefined : { width: rightPanelWidth }}
        >
          <div className="panel-header panel-header-tabs" data-testid="right-panel-tabs" role="tablist" aria-label="Right panel views">
            <button
              className="panel-collapse-btn"
              onClick={() => setRightPanelCollapsed(!rightPanelCollapsed)}
              title={rightPanelCollapsed ? "Expand panel" : "Collapse panel"}
            >
              {rightPanelCollapsed ? "\u25C0" : "\u25B6"}
            </button>
            {!rightPanelCollapsed && (
              <>
                <button
                  className={`panel-tab ${rightPanelTab === "activity" ? "active" : ""}`}
                  onClick={() => setRightPanelTab("activity")}
                  data-testid="activity-tab"
                  role="tab"
                  aria-selected={rightPanelTab === "activity"}
                  title="LLM"
                >
                  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    <polyline points="1,8 4,8 6,3 8,13 10,6 12,8 15,8" />
                  </svg>
                  {(activityJob || activityTimeline.length > 0) && (
                    <span className="panel-tab-count">{activityTimeline.length || 1}</span>
                  )}
                </button>
                <button
                  className={`panel-tab ${rightPanelTab === "comments" ? "active" : ""}`}
                  onClick={() => setRightPanelTab("comments")}
                  data-testid="comments-tab"
                  role="tab"
                  aria-selected={rightPanelTab === "comments"}
                  title="Comments"
                >
                  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M2,2 h12 a1,1 0 0 1 1,1 v7 a1,1 0 0 1 -1,1 h-7 l-3,3 v-3 h-2 a1,1 0 0 1 -1,-1 v-7 a1,1 0 0 1 1,-1 z" />
                  </svg>
                  {comments.length > 0 && (
                    <span className="panel-tab-count">{comments.length}</span>
                  )}
                </button>
                <button
                  className={`panel-tab ${rightPanelTab === "annotations" ? "active" : ""}`}
                  onClick={() => setRightPanelTab("annotations")}
                  data-testid="annotations-tab"
                  role="tab"
                  aria-selected={rightPanelTab === "annotations"}
                  title="Info"
                >
                  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    <rect x="2" y="1" width="12" height="14" rx="1.5" />
                    <line x1="5" y1="5" x2="11" y2="5" />
                    <line x1="5" y1="8" x2="11" y2="8" />
                    <line x1="5" y1="11" x2="9" y2="11" />
                  </svg>
                </button>
                <button
                  className={`panel-tab ${rightPanelTab === "source" ? "active" : ""}`}
                  onClick={() => setRightPanelTab("source")}
                  data-testid="source-tab"
                  role="tab"
                  aria-selected={rightPanelTab === "source"}
                  title="Subsystem Source"
                >
                  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    <polyline points="5,3 1,8 5,13" />
                    <polyline points="11,3 15,8 11,13" />
                    <line x1="9" y1="2" x2="7" y2="14" />
                  </svg>
                </button>
              </>
            )}
          </div>
          {!rightPanelCollapsed && (
          <div className="panel-body panel-body-right">
            {rightPanelTab === "activity"
              ? <ActivityTab />
              : rightPanelTab === "source"
                ? <SourceTab />
                : rightPanelTab === "comments"
                ? <CommentsTab />
                  : <AnnotationsTab />}

            {(annotating || deepAnalyzing || refining) && rightPanelTab === "activity" && (
              <div className="annotation-section llm-loading">
                <span className="spinner" />
                {annotating
                  ? "Generating overview..."
                  : deepAnalyzing
                    ? "Analyzing flow group..."
                    : "Refining groups..."}
              </div>
            )}

            {!aiAccessReady && llmSettings && (
              <div className="annotation-section llm-loading llm-setup-cta">
                <span>Connect {PROVIDER_LABELS.codex}, {PROVIDER_LABELS.claude}, or a direct API key to unlock summaries and refinement.</span>
                <button className="btn" onClick={() => openAiSetup("recommended")}>
                  Setup AI
                </button>
              </div>
            )}
          </div>
          )}
        </aside>
    </>
  );
}
