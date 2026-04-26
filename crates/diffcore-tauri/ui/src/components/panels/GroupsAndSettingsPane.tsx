import { useAppContext } from "../../hooks/AppContext";
import { LeftPane } from "./LeftPane";
import { SettingsPanel } from "../modals/SettingsPanel";

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
          className={`panel-tab ${rightmostTab === "settings" ? "active" : ""}`}
          onClick={() => setRightmostTab("settings")}
          role="tab"
          aria-selected={rightmostTab === "settings"}
          title="Settings"
        >
          Settings
        </button>
      </div>

      <div className="panel-body" style={{ flex: 1, minHeight: 0, overflow: "hidden" }}>
        {rightmostTab === "groups" ? <LeftPane embedded /> : <SettingsPanel embedded />}
      </div>
    </aside>
  );
}
