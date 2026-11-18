// AboutMenu — toolbar dropdown with two-step update flow:
//   1. "Check for Update" → polls GitHub, shows result.
//   2. "Update Now" (shown only when update is available) → downloads,
//      installs, and restarts the app.

import { useEffect, useRef, useState } from "react";
import { getAppVersion, checkUpdate, installUpdate } from "../api/updater";

type UpdateState =
  | { phase: "idle" }
  | { phase: "checking" }
  | { phase: "up-to-date" }
  | { phase: "available"; latestVersion: string }
  | { phase: "installing" }
  | { phase: "error"; message: string };

export default function AboutMenu() {
  const [open, setOpen] = useState(false);
  const [version, setVersion] = useState<string>("…");
  const [updateState, setUpdateState] = useState<UpdateState>({ phase: "idle" });
  const menuRef = useRef<HTMLDivElement>(null);

  // Load version once on mount.
  useEffect(() => {
    getAppVersion().then(setVersion).catch(() => setVersion("?"));
  }, []);

  // Reset update state when dropdown closes so it feels fresh next time.
  useEffect(() => {
    if (!open) setUpdateState({ phase: "idle" });
  }, [open]);

  // Close when clicking outside.
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  const handleCheck = async () => {
    setUpdateState({ phase: "checking" });
    try {
      const info = await checkUpdate();
      if (info.available && info.latest_version) {
        setUpdateState({ phase: "available", latestVersion: info.latest_version });
      } else {
        setUpdateState({ phase: "up-to-date" });
      }
    } catch (err) {
      setUpdateState({
        phase: "error",
        message: err instanceof Error ? err.message : String(err),
      });
    }
  };

  const handleInstall = async () => {
    setUpdateState((prev) =>
      prev.phase === "available"
        ? { phase: "installing" }
        : prev
    );
    try {
      await installUpdate();
      // installUpdate restarts the app — this line is never reached on success.
    } catch (err) {
      setUpdateState({
        phase: "error",
        message: err instanceof Error ? err.message : String(err),
      });
    }
  };

  const isChecking = updateState.phase === "checking";
  const isInstalling = updateState.phase === "installing";

  return (
    <div className="about-menu" ref={menuRef}>
      <button
        className="btn btn--ghost about-menu__trigger"
        title="About Lightman"
        aria-label="About Lightman"
        aria-expanded={open}
        aria-haspopup="true"
        onClick={() => setOpen((v) => !v)}
      >
        <span className="about-menu__version-badge">v{version}</span>
      </button>

      {open && (
        <div className="about-menu__dropdown" role="dialog" aria-label="About Lightman">
          {/* App identity row */}
          <div className="about-menu__app-row">
            <img src="/logo.png" alt="" width="28" height="28" className="about-menu__app-icon" />
            <div>
              <div className="about-menu__app-name">Lightman</div>
              <div className="about-menu__app-version">Version {version}</div>
            </div>
          </div>

          <div className="about-menu__divider" />

          {/* Status messages */}
          {updateState.phase === "up-to-date" && (
            <p className="about-menu__status about-menu__status--ok">
              ✓ Up to date
            </p>
          )}
          {updateState.phase === "available" && (
            <p className="about-menu__status about-menu__status--available">
              ↑ Version {updateState.latestVersion} available
            </p>
          )}
          {updateState.phase === "error" && (
            <p
              className="about-menu__status about-menu__status--error"
              title={updateState.message}
            >
              ✗ {updateState.message}
            </p>
          )}
          {isInstalling && (
            <p className="about-menu__status">Downloading and installing…</p>
          )}

          {/* Primary action buttons */}
          {updateState.phase !== "available" && !isInstalling && (
            <button
              className="btn btn--secondary about-menu__update-btn"
              onClick={handleCheck}
              disabled={isChecking}
            >
              {isChecking ? (
                <>
                  <span className="about-menu__spinner about-menu__spinner--dark" aria-hidden="true" />
                  Checking…
                </>
              ) : (
                "Check for Update"
              )}
            </button>
          )}

          {updateState.phase === "available" && (
            <div className="about-menu__btn-group">
              <button
                className="btn btn--primary about-menu__update-btn"
                onClick={handleInstall}
              >
                Update Now
              </button>
              <button
                className="btn btn--ghost about-menu__update-btn"
                onClick={handleCheck}
              >
                Re-check
              </button>
            </div>
          )}

          {isInstalling && (
            <button className="btn btn--primary about-menu__update-btn" disabled>
              <span className="about-menu__spinner" aria-hidden="true" />
              Installing…
            </button>
          )}
        </div>
      )}
    </div>
  );
}
