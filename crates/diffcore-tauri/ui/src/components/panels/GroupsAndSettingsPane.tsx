import { useAppContext } from "../../hooks/AppContext";
import { RightmostPane } from "./RightmostPane";
import { SettingsPanel } from "../modals/SettingsPanel";
import { AiOverlayTab } from "../tabs/AiOverlayTab";

export function GroupsAndSettingsPane() {
  const { rightmostTab, setRightmostTab, groupsPanelWidth } = useAppContext();

  return (
    <aside className="panel panel-rightmost" style={{ width: groupsPanelWidth }}>
      <div className="panel-header panel-header-tabs" role="tablist" aria-label="Groups and settings">
        <button
          className={`panel-tab ${rightmostTab === "groups" ? "active" : ""}`}
          onClick={() => setRightmostTab("groups")}
          role="tab"
          aria-selected={rightmostTab === "groups"}
          title="Flow Groups"
        >
          Groups
        </button>
        <button
          className={`panel-tab ${rightmostTab === "ai" ? "active" : ""}`}
          onClick={() => setRightmostTab("ai")}
          role="tab"
          aria-selected={rightmostTab === "ai"}
          title="AI"
        >
          AI
        </button>
        <button
          className={`panel-tab ${rightmostTab === "settings" ? "active" : ""}`}
          onClick={() => setRightmostTab("settings")}
          role="tab"
          aria-selected={rightmostTab === "settings"}
          title="Settings"
        >
          <span className="sr-only">Settings</span>
          <svg
            width="16"
            height="16"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d="M6.6 1.3l.7 1.7a5.7 5.7 0 0 1 1.4 0l.7-1.7 1.5.9-.8 1.7c.4.4.7.8 1 1.2l1.8-.2.3 1.7-1.7.6a5.8 5.8 0 0 1 0 1.4l1.7.6-.3 1.7-1.8-.2c-.3.4-.6.8-1 1.2l.8 1.7-1.5.9-.7-1.7a5.7 5.7 0 0 1-1.4 0l-.7 1.7-1.5-.9.8-1.7c-.4-.4-.7-.8-1-1.2l-1.8.2-.3-1.7 1.7-.6a5.8 5.8 0 0 1 0-1.4l-1.7-.6.3-1.7 1.8.2c.3-.4.6-.8 1-1.2l-.8-1.7 1.5-.9z" />
            <circle cx="8" cy="8" r="2.2" />
          </svg>
        </button>
      </div>

      <div className="rightmost-tab-content">
        {rightmostTab === "groups"
          ? <RightmostPane embedded />
          : rightmostTab === "settings"
            ? <SettingsPanel embedded />
            : <AiOverlayTab />}
      </div>
    </aside>
  );
}
