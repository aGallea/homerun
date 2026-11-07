import { FormEvent, useEffect, useRef, useState } from "react";
import type { RunnerInfo } from "../api/types";
import { api } from "../api/commands";

interface RenameRunnerDialogProps {
  runner: RunnerInfo;
  onClose: () => void;
}

export function RenameRunnerDialog({ runner, onClose }: RenameRunnerDialogProps) {
  const [value, setValue] = useState(runner.config.display_name ?? "");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const currentValue = runner.config.display_name ?? null;
  const normalizedValue = value.trim() || null;
  const unchanged = normalizedValue === currentValue;

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape" && !saving) onClose();
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [onClose, saving]);

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    if (saving || unchanged) return;

    setSaving(true);
    setError(null);
    try {
      await api.updateRunnerDisplayName(runner.config.id, normalizedValue);
      onClose();
    } catch (e) {
      setError(String(e));
      setSaving(false);
    }
  }

  return (
    <div className="dialog-overlay" onClick={saving ? undefined : onClose}>
      <form className="dialog" onSubmit={handleSubmit} onClick={(e) => e.stopPropagation()}>
        <h3 className="dialog-title">Rename Runner</h3>
        <p className="dialog-message">
          This changes only the name shown in HomeRun. The GitHub runner name{" "}
          <strong>{runner.config.name}</strong> will not change. Clear the field to restore the
          GitHub name.
        </p>
        <input
          ref={inputRef}
          className="input"
          value={value}
          maxLength={100}
          placeholder={runner.config.name}
          aria-label="Runner display name"
          disabled={saving}
          onChange={(event) => setValue(event.target.value)}
          style={{ width: "100%", marginBottom: 12 }}
        />
        {error && <div className="error-banner">{error}</div>}
        <div className="dialog-actions">
          <button type="button" className="btn" onClick={onClose} disabled={saving}>
            Cancel
          </button>
          <button type="submit" className="btn btn-primary" disabled={saving || unchanged}>
            {saving ? "Saving…" : normalizedValue ? "Save Display Name" : "Use GitHub Name"}
          </button>
        </div>
      </form>
    </div>
  );
}
