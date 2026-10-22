// App shell — three-panel layout: sidebar | request builder | response viewer.
//
// Task 15.2: App shell with toolbar + three-panel responsive layout.
// Task 20:   History panel added as a tab inside the left sidebar.
// Requirements: 8.2, 4.2, 4.3

import { useState, useCallback, useRef } from "react";
import "./App.css";
import Toolbar from "./components/Toolbar";
import Sidebar from "./components/Sidebar";
import HistoryPanel from "./components/HistoryPanel";
import RequestBuilder from "./components/RequestBuilder";
import ResponseViewer from "./components/ResponseViewer";

// ---------------------------------------------------------------------------
// ResizeDivider — generic drag handle for resizing panels
//
// side="left"  → panel is to the LEFT of the divider; width = clientX
// side="right" → panel is to the RIGHT of the divider; width = innerWidth - clientX
// ---------------------------------------------------------------------------

const SIDEBAR_MIN = 180;
const SIDEBAR_MAX = 480;
const RESPONSE_MIN = 240;
const RESPONSE_MAX_RATIO = 0.7; // max 70% of window width

function ResizeDivider({
  side,
  onResize,
  ariaLabel,
  panelRef,
}: {
  side: "left" | "right";
  onResize: (width: number) => void;
  ariaLabel: string;
  panelRef?: React.RefObject<HTMLElement | null>;
}) {
  const dragging = useRef(false);

  const onPointerDown = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      e.preventDefault();
      dragging.current = true;
      const el = e.currentTarget as HTMLDivElement;
      el.setPointerCapture(e.pointerId);
      el.classList.add("resize-divider--dragging");
      // Disable CSS transition on the target panel while dragging
      panelRef?.current?.classList.add("is-resizing");

      const onPointerMove = (ev: PointerEvent) => {
        if (!dragging.current) return;
        let newWidth: number;
        if (side === "left") {
          newWidth = Math.max(SIDEBAR_MIN, Math.min(SIDEBAR_MAX, ev.clientX));
        } else {
          const maxWidth = Math.floor(window.innerWidth * RESPONSE_MAX_RATIO);
          newWidth = Math.max(RESPONSE_MIN, Math.min(maxWidth, window.innerWidth - ev.clientX));
        }
        onResize(newWidth);
      };

      const onPointerUp = () => {
        dragging.current = false;
        el.classList.remove("resize-divider--dragging");
        panelRef?.current?.classList.remove("is-resizing");
        document.removeEventListener("pointermove", onPointerMove);
        document.removeEventListener("pointerup", onPointerUp);
      };

      document.addEventListener("pointermove", onPointerMove);
      document.addEventListener("pointerup", onPointerUp);
    },
    [side, onResize, panelRef]
  );

  return (
    <div
      className="resize-divider"
      role="separator"
      aria-orientation="vertical"
      aria-label={ariaLabel}
      onPointerDown={onPointerDown}
    />
  );
}

type LeftTab = "collections" | "history";

function App() {
  const [leftTab, setLeftTab] = useState<LeftTab>("collections");
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [sidebarWidth, setSidebarWidth] = useState(260);
  const [responseWidth, setResponseWidth] = useState(400);
  const sidebarRef = useRef<HTMLDivElement>(null);

  return (
    <div className="app-shell">
      {/* Top toolbar — brand + environment selector */}
      <Toolbar sidebarOpen={sidebarOpen} onToggleSidebar={() => setSidebarOpen((v) => !v)} />

      {/* Three-panel body */}
      <div className="app-body">
        {/* Left — Collections / History */}
        <div
          ref={sidebarRef}
          className={`app-left-panel${sidebarOpen ? "" : " app-left-panel--hidden"}`}
          style={sidebarOpen ? { width: sidebarWidth } : undefined}
        >
          {/* Tab switcher */}
          <div className="left-panel__tabs" role="tablist" aria-label="Left panel">
            <button
              className={`left-panel__tab${leftTab === "collections" ? " left-panel__tab--active" : ""}`}
              role="tab"
              aria-selected={leftTab === "collections"}
              aria-controls="left-panel-collections"
              id="tab-collections"
              onClick={() => setLeftTab("collections")}
            >
              Collections
            </button>
            <button
              className={`left-panel__tab${leftTab === "history" ? " left-panel__tab--active" : ""}`}
              role="tab"
              aria-selected={leftTab === "history"}
              aria-controls="left-panel-history"
              id="tab-history"
              onClick={() => setLeftTab("history")}
            >
              History
            </button>
          </div>

          {/* Panel content */}
          <div
            id="left-panel-collections"
            role="tabpanel"
            aria-labelledby="tab-collections"
            hidden={leftTab !== "collections"}
            className="left-panel__content"
          >
            <Sidebar />
          </div>
          <div
            id="left-panel-history"
            role="tabpanel"
            aria-labelledby="tab-history"
            hidden={leftTab !== "history"}
            className="left-panel__content"
          >
            <HistoryPanel />
          </div>
        </div>

        {/* Drag handle — resize collection panel */}
        {sidebarOpen && (
          <ResizeDivider side="left" onResize={setSidebarWidth} ariaLabel="Resize collection panel" panelRef={sidebarRef} />
        )}

        {/* Centre — Request builder */}
        <RequestBuilder />

        {/* Drag handle — resize response panel */}
        <ResizeDivider side="right" onResize={setResponseWidth} ariaLabel="Resize response panel" />

        {/* Right — Response viewer */}
        <ResponseViewer style={{ width: responseWidth }} />
      </div>
    </div>
  );
}

export default App;
