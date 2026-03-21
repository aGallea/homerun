import type { CreateRunnerRequest, RunnerInfo } from "../api/types";

interface NewRunnerWizardProps {
  onClose: () => void;
  onCreate: (req: CreateRunnerRequest) => Promise<RunnerInfo>;
}

export function NewRunnerWizard({ onClose }: NewRunnerWizardProps) {
  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog" onClick={(e) => e.stopPropagation()}>
        <h3 className="dialog-title">New Runner</h3>
        <p className="dialog-message">Wizard coming in Task 6.</p>
        <div className="dialog-actions">
          <button className="btn" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
