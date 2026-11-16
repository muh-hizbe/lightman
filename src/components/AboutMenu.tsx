// AboutMenu — toolbar dropdown showing the current app version and an
// "Update" button that checks for a newer GitHub release, downloads it,
// installs it, and restarts the app automatically.

import { useEffect, useRef, useState } from "react";
import { getAppVersion, checkAndInstallUpdate } from "../api/updater";

type UpdateState =
  | { phase: "idle" }
  | { phase: "checking" }
  | { phase: "up-to-date" }
  | { phase: "downloading" }
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

  const handleUpdate = async () => {
    setUpdateState({ phase: "checking" });
    try {
      const info = await checkAndInstallUpdate();
      // If we get here, no update was available (an update triggers restart).
      if (!info.available) {
        setUpdateState({ phase: "up-to-date" });
      }
    } catch (err) {
      setUpdateState({
        phase: "error",
        message: err instanceof Error ? err.message : String(err),
      });
    }
  };

  const isWorking =
    updateState.phase === "checking" || updateState.phase === "downloading";

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
          <div className="about-menu__app-row">
            <img src="/logo.png" alt="" width="28" height="28" className="about-menu__app-icon" />
            <div>
              <div className="about-menu__app-name">Lightman</div>
              <div className="about-menu__app-version">Version {version}</div>
            </div>
          </div>

          <div className="about-menu__divider" />

          {/* Update status feedback */}
          {updateState.phase === "up-to-date" && (
            <p className="about-menu__status about-menu__status--ok">
              ✓ Sudah versi terbaru
            </p>
          )}
          {updateState.phase === "error" && (
            <p className="about-menu__status about-menu__status--error" title={updateState.message}>
              ✗ {updateState.message}
            </p>
          )}
          {updateState.phase === "checking" && (
            <p className="about-menu__status">Memeriksa pembaruan…</p>
          )}
          {updateState.phase === "downloading" && (
            <p className="about-menu__status">Mengunduh dan memasang…</p>
          )}

          <button
            className="btn btn--primary about-menu__update-btn"
            onClick={handleUpdate}
            disabled={isWorking}
          >
            {isWorking ? (
              <>
                <span className="about-menu__spinner" aria-hidden="true" />
                {updateState.phase === "checking" ? "Memeriksa…" : "Mengunduh…"}
              </>
            ) : (
              "Cek & Pasang Update"
            )}
          </button>

          <p className="about-menu__hint">
            Jika tersedia, update akan diunduh dan app akan dimulai ulang otomatis.
          </p>
        </div>
      )}
    </div>
  );
}
