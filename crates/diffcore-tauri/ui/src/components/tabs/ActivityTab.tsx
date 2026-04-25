/**
 * ActivityTab — live LLM activity stream shown in the right panel.
 *
 * Displays two regions:
 *  1. Hero section — job status, provider badge, event stats, and the
 *     refinement verdict callout.
 *  2. Events section — paginated event feed (stream or all), direct-API
 *     mode banner, and expandable event cards.
 *
 * All state is consumed from AppContext so no props are required.
 */
import type { LlmProvider } from "../../types";
import { formatActivityTimestamp } from "../../utils/activityUtils";
import { PROVIDER_LABELS } from "../../utils/llmUtils";
import { useAppContext } from "../../hooks/AppContext";

/** Maximum number of events shown in "stream" view. */
const ACTIVITY_STREAM_LIMIT = 10;

export function ActivityTab() {
  const {
    activityJob,
    activityTimeline,
    activityStats,
    activityEventProvider,
    activitySupportsToolStreaming,
    activityIsDirectApi,
    refinementVerdict,
    showDirectApiBanner,
    recommendedSubscriptionProvider,
    inspectedActivityId,
    setInspectedActivityId,
    activityLogRef,
    activityError,
    activityViewMode,
    setActivityViewMode,
    visibleActivityTimeline,
    dismissDirectApiNotice,
    activatePreferredActivityProvider,
  } = useAppContext();

  return (
    <div className="activity-tab-shell">
      <div className="activity-tab-region activity-tab-region-hero">
      <div className="annotation-section activity-hero-section" data-testid="activity-panel">
        <div className="activity-hero">
          <div>
            <p className="activity-hero-eyebrow">AI Activity</p>
            <p className="activity-hero-title">
              {activityJob
                ? activityJob.title
                : activityTimeline.length > 0
                  ? "Latest AI run"
                  : "No AI activity yet"}
            </p>
            <p className="activity-hero-subtitle">
              {activityJob
                ? `${PROVIDER_LABELS[activityJob.provider as LlmProvider] ?? activityJob.provider}/${activityJob.model}`
                : activityTimeline.length > 0 && activityEventProvider
                  ? `Latest stream captured from ${PROVIDER_LABELS[activityEventProvider as LlmProvider] ?? activityEventProvider}${activitySupportsToolStreaming ? " with live repo access." : "."}`
                  : activityEventProvider
                    ? `${PROVIDER_LABELS[activityEventProvider as LlmProvider] ?? activityEventProvider}${activitySupportsToolStreaming ? " can stream repo activity live." : " is running in direct API mode."}`
                  : "Run Summarize PR, Analyze This Flow, or Refine to inspect AI work."}
            </p>
          </div>
          {activityJob ? (
            <span className="activity-live-badge">Live</span>
          ) : activityTimeline.length > 0 ? (
            <span className="activity-live-badge activity-live-idle">Saved</span>
          ) : null}
        </div>

        {(activityJob || activityTimeline.length > 0) && (
          <div
            className={`activity-stats ${activityIsDirectApi ? "activity-stats-events-only" : ""}`}
            data-testid="activity-stats"
          >
            <div className="activity-stat">
              <span className="activity-stat-value">{activityStats.total}</span>
              <span className="activity-stat-label">events</span>
            </div>
            {/*
              Search / reads / commands tiles are hidden in direct-API mode
              because hosted APIs (OpenAI / Anthropic / Gemini) don't emit
              tool events — leaving them visible just shows three stale 0s.
            */}
            {!activityIsDirectApi && (
              <>
                <div className="activity-stat">
                  <span className="activity-stat-value">{activityStats.search}</span>
                  <span className="activity-stat-label">search</span>
                </div>
                <div className="activity-stat">
                  <span className="activity-stat-value">{activityStats.read}</span>
                  <span className="activity-stat-label">reads</span>
                </div>
                <div className="activity-stat">
                  <span className="activity-stat-value">{activityStats.command}</span>
                  <span className="activity-stat-label">commands</span>
                </div>
              </>
            )}
          </div>
        )}

        {activitySupportsToolStreaming && activityEventProvider && (
          <div className="activity-callout activity-callout-positive">
            <p className="activity-callout-title">Live repo activity enabled</p>
            <p className="activity-callout-body">
              {PROVIDER_LABELS[activityEventProvider as LlmProvider] ?? activityEventProvider} can inspect the repo directly,
              so file reads, searches, and shell commands stream here while you wait.
            </p>
          </div>
        )}

        {refinementVerdict && (
          <div className={`activity-callout ${refinementVerdict.hadChanges ? "activity-callout-positive" : ""}`} data-testid="activity-refinement-verdict">
            <p className="activity-callout-title">{refinementVerdict.title}</p>
            <p className="activity-callout-body">
              {PROVIDER_LABELS[refinementVerdict.provider as LlmProvider] ?? refinementVerdict.provider}/{refinementVerdict.model}
            </p>
            {refinementVerdict.reasoning && (
              <p className="activity-callout-body">{refinementVerdict.reasoning}</p>
            )}
          </div>
        )}
      </div>
      </div>

      <div className="activity-tab-divider" role="presentation" aria-hidden="true" />

      <div className="activity-tab-region activity-tab-region-events">
      <div className="annotation-section activity-section" data-testid="activity-log-panel">
        {showDirectApiBanner && (
          <div className="activity-events-banner" data-testid="activity-direct-api-note" role="status">
            <div className="activity-events-banner-body">
              <p className="activity-events-banner-title">Direct API mode</p>
              <p className="activity-events-banner-text">
                OpenAI, Anthropic, and Gemini only emit high-level progress.
                File reads, grep searches, and shell steps appear when Diffcore routes
                the job through Codex CLI or Claude Code.
              </p>
              {recommendedSubscriptionProvider && (
                <button
                  className="btn btn-sm activity-events-banner-action"
                  onClick={activatePreferredActivityProvider}
                >
                  Use {PROVIDER_LABELS[recommendedSubscriptionProvider]}
                </button>
              )}
            </div>
            <button
              type="button"
              className="activity-events-banner-dismiss"
              onClick={dismissDirectApiNotice}
              aria-label="Dismiss direct API mode notice"
              title="Dismiss"
            >
              &times;
            </button>
          </div>
        )}
        <div className="activity-log-header">
          <div>
            <h3>{activityViewMode === "stream" ? "Live Stream" : "All Events"}</h3>
            <p className="activity-log-subtitle">
              {activityViewMode === "stream"
                ? activityTimeline.length > ACTIVITY_STREAM_LIMIT
                  ? `Showing the latest ${ACTIVITY_STREAM_LIMIT} of ${activityTimeline.length} events.`
                  : "Newest activity lands at the bottom of the stream."
                : `${activityTimeline.length} events captured for the current run.`}
            </p>
          </div>
          <div className="activity-log-actions">
            {activityError && <span className="activity-error-inline">{activityError}</span>}
            <div className="activity-view-switch" role="tablist" aria-label="Activity views">
              <button
                className={`activity-view-tab ${activityViewMode === "stream" ? "active" : ""}`}
                type="button"
                role="tab"
                aria-selected={activityViewMode === "stream"}
                data-testid="activity-view-stream-tab"
                onClick={() => setActivityViewMode("stream")}
              >
                Stream
                <span className="activity-view-count">{Math.min(activityTimeline.length, ACTIVITY_STREAM_LIMIT)}</span>
              </button>
              <button
                className={`activity-view-tab ${activityViewMode === "all" ? "active" : ""}`}
                type="button"
                role="tab"
                aria-selected={activityViewMode === "all"}
                data-testid="activity-view-all-tab"
                onClick={() => setActivityViewMode("all")}
              >
                All events
                <span className="activity-view-count">{activityTimeline.length}</span>
              </button>
            </div>
          </div>
        </div>
        <div className="activity-feed-shell">
          <div
            ref={activityLogRef}
            className={`activity-log activity-log-rich activity-log-${activityViewMode}`}
            data-testid="activity-log"
            data-activity-view={activityViewMode}
          >
            {visibleActivityTimeline.length === 0 && (
              <div className="activity-empty">
                Activity will appear here once an AI job starts.
              </div>
            )}
            {visibleActivityTimeline.map(({ id, entry, presentation }, index) => {
              const isExpanded = inspectedActivityId === id;
              const hasExtraContent = Boolean(
                presentation.eventTypeLabel
                || presentation.detail
                || presentation.payloadText,
              );

              return (
                <div
                  key={id}
                  className={`activity-card activity-card-${presentation.kind} activity-${entry.level} ${activityViewMode === "stream" && index === visibleActivityTimeline.length - 1 ? "activity-card-latest" : ""} ${isExpanded ? "activity-card-expanded" : ""}`}
                  data-testid="activity-entry"
                >
                  <button
                    type="button"
                    className="activity-card-trigger"
                    aria-pressed={isExpanded}
                    onClick={() => setInspectedActivityId(id)}
                  >
                    <div className="activity-card-meta">
                      <span className={`activity-kind-badge activity-kind-${presentation.kind}`}>{presentation.badge}</span>
                      <span className="activity-source-pill">{presentation.sourceLabel}</span>
                      <span className="activity-time">{formatActivityTimestamp(entry.timestamp_ms)}</span>
                    </div>
                    <div className="activity-card-body">
                      <p className="activity-card-title">{presentation.title}</p>
                      {presentation.subject && (
                        <p className="activity-card-subject">{presentation.subject}</p>
                      )}
                      {hasExtraContent && (
                        <div className="activity-card-hints">
                          {presentation.eventTypeLabel && (
                            <span className="activity-card-hint" title={presentation.eventTypeLabel}>Event</span>
                          )}
                          {presentation.detail && (
                            <span className="activity-card-hint" title={presentation.detail}>{presentation.detailLabel ?? "Detail"}</span>
                          )}
                          {presentation.payloadText && (
                            <span className="activity-card-hint" title={presentation.payloadText}>
                              Payload{presentation.payloadSummary ? ` · ${presentation.payloadSummary}` : ""}
                            </span>
                          )}
                        </div>
                      )}
                    </div>
                  </button>
                </div>
              );
            })}
          </div>

        </div>
      </div>
      </div>
    </div>
  );
}
