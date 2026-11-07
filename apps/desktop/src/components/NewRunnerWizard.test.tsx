import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import { NewRunnerWizard } from "./NewRunnerWizard";
import { makeRepo } from "../test/factories";
import { api } from "../api/commands";
import { AuthProvider } from "../hooks/AuthContext";

vi.mock("../api/commands", () => ({
  api: {
    listRepos: vi.fn(),
    getAuthStatus: vi.fn(),
    getDockerStatus: vi.fn(),
  },
}));

const mockRepos = [
  makeRepo({ full_name: "org/frontend", id: 1 }),
  makeRepo({ full_name: "org/backend", id: 2, private: true }),
  makeRepo({ full_name: "other/utils", id: 3 }),
];

async function renderWizard(props: Partial<Parameters<typeof NewRunnerWizard>[0]> = {}) {
  const defaultProps = {
    onClose: vi.fn(),
    onCreate: vi.fn().mockResolvedValue({
      config: {
        id: "r1",
        name: "test-runner",
        repo_owner: "org",
        repo_name: "frontend",
        labels: [],
        mode: "app",
        work_dir: "/tmp",
      },
      state: "creating",
      pid: null,
      uptime_secs: null,
      jobs_completed: 0,
      jobs_failed: 0,
      current_job: null,
      job_started_at: null,
      estimated_job_duration_secs: null,
    }),
    onCreateBatch: vi.fn().mockResolvedValue({ group_id: "g1", runners: [], errors: [] }),
    ...props,
  };
  let result!: ReturnType<typeof render>;
  await act(async () => {
    result = render(
      <AuthProvider>
        <NewRunnerWizard {...defaultProps} />
      </AuthProvider>,
    );
  });
  return {
    ...result,
    props: defaultProps,
  };
}

beforeEach(() => {
  vi.mocked(api.listRepos).mockResolvedValue(mockRepos);
  vi.mocked(api.getAuthStatus).mockResolvedValue({
    authenticated: true,
    user: { login: "test", avatar_url: "" },
  });
  vi.mocked(api.getDockerStatus).mockResolvedValue({ available: true });
});

describe("NewRunnerWizard", () => {
  it("renders step 1 (Select Repository) initially", async () => {
    await renderWizard();
    expect(screen.getByPlaceholderText("Search repositories...")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByText("org/frontend")).toBeInTheDocument();
    });
  });

  it("filters repos by search input", async () => {
    await renderWizard();
    await waitFor(() => expect(screen.getByText("org/frontend")).toBeInTheDocument());

    fireEvent.change(screen.getByPlaceholderText("Search repositories..."), {
      target: { value: "backend" },
    });
    expect(screen.queryByText("org/frontend")).not.toBeInTheDocument();
    expect(screen.getByText("org/backend")).toBeInTheDocument();
  });

  it("advances to step 2 on repo selection", async () => {
    await renderWizard();
    await waitFor(() => expect(screen.getByText("org/frontend")).toBeInTheDocument());

    fireEvent.click(screen.getByText("org/frontend"));
    // Step 2 shows name input
    expect(screen.getByLabelText("Name")).toBeInTheDocument();
  });

  it("disables Next on step 2 when name is empty (single mode)", async () => {
    await renderWizard();
    await waitFor(() => expect(screen.getByText("org/frontend")).toBeInTheDocument());

    fireEvent.click(screen.getByText("org/frontend"));
    // Clear the auto-generated name
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "" } });
    const nextBtn = screen.getByRole("button", { name: "Next" });
    expect(nextBtn).toBeDisabled();
  });

  it("advances to step 3 (Launch) and calls onCreate on single create", async () => {
    const { props } = await renderWizard();
    await waitFor(() => expect(screen.getByText("org/frontend")).toBeInTheDocument());

    // Step 1: select repo
    fireEvent.click(screen.getByText("org/frontend"));

    // Step 2: name is auto-filled, click Next
    fireEvent.click(screen.getByRole("button", { name: "Next" }));

    // Step 3: Launch
    expect(screen.getByText("Review the configuration before launching.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Launch Runner" }));

    await waitFor(() => {
      expect(props.onCreate).toHaveBeenCalledTimes(1);
    });
    expect(props.onCreate).toHaveBeenCalledWith(
      expect.objectContaining({
        repo_full_name: "org/frontend",
        mode: "app",
      }),
    );
  });

  it("selects Container mode and sends the image in onCreate", async () => {
    const { props } = await renderWizard();
    await waitFor(() => expect(screen.getByText("org/frontend")).toBeInTheDocument());

    fireEvent.click(screen.getByText("org/frontend"));
    await waitFor(() => expect(screen.getByText("Container")).toBeInTheDocument());

    fireEvent.click(screen.getByText("Container"));
    const imageInput = screen.getByLabelText("Image") as HTMLInputElement;
    expect(imageInput.value).toBe("ghcr.io/agallea/homerun-runner:ubuntu-24.04");
    fireEvent.change(imageInput, { target: { value: "ghcr.io/acme/custom-runner:latest" } });

    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    fireEvent.click(screen.getByRole("button", { name: "Launch Runner" }));

    await waitFor(() => {
      expect(props.onCreate).toHaveBeenCalledTimes(1);
    });
    expect(props.onCreate).toHaveBeenCalledWith(
      expect.objectContaining({
        mode: "container",
        container: { image: "ghcr.io/acme/custom-runner:latest" },
      }),
    );
  });

  it("Rust preset fills the rust image and rust label", async () => {
    const { props } = await renderWizard();
    await waitFor(() => expect(screen.getByText("org/frontend")).toBeInTheDocument());

    fireEvent.click(screen.getByText("org/frontend"));
    await waitFor(() => expect(screen.getByText("Container")).toBeInTheDocument());
    fireEvent.click(screen.getByText("Container"));

    fireEvent.click(screen.getByRole("button", { name: "Rust" }));
    const imageInput = screen.getByLabelText("Image") as HTMLInputElement;
    expect(imageInput.value).toBe("ghcr.io/agallea/homerun-runner:rust");

    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    fireEvent.click(screen.getByRole("button", { name: "Launch Runner" }));

    await waitFor(() => expect(props.onCreate).toHaveBeenCalledTimes(1));
    expect(props.onCreate).toHaveBeenCalledWith(
      expect.objectContaining({
        mode: "container",
        container: { image: "ghcr.io/agallea/homerun-runner:rust" },
        labels: expect.arrayContaining(["rust"]),
      }),
    );
  });

  it("disables Container mode when Docker isn't available", async () => {
    vi.mocked(api.getDockerStatus).mockResolvedValue({ available: false });
    await renderWizard();
    await waitFor(() => expect(screen.getByText("org/frontend")).toBeInTheDocument());

    fireEvent.click(screen.getByText("org/frontend"));
    await waitFor(() => expect(screen.getByText("Container")).toBeInTheDocument());

    const containerButton = screen.getByText("Container").closest("button");
    expect(containerButton).toBeDisabled();
  });

  it("calls onCreateBatch when count > 1", async () => {
    const { props } = await renderWizard();
    await waitFor(() => expect(screen.getByText("org/frontend")).toBeInTheDocument());

    fireEvent.click(screen.getByText("org/frontend"));

    // Pick count = 3 — use getAllByText and pick the button (not the step indicator)
    const threeButtons = screen.getAllByText("3");
    const countBtn = threeButtons.find((el) => el.tagName === "BUTTON");
    fireEvent.click(countBtn!);

    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    fireEvent.click(screen.getByRole("button", { name: "Launch 3 Runners" }));

    await waitFor(() => {
      expect(props.onCreateBatch).toHaveBeenCalledTimes(1);
    });
    expect(props.onCreateBatch).toHaveBeenCalledWith(
      expect.objectContaining({
        repo_full_name: "org/frontend",
        count: 3,
        mode: "app",
      }),
    );
  });

  it("Back button returns to previous step", async () => {
    await renderWizard();
    await waitFor(() => expect(screen.getByText("org/frontend")).toBeInTheDocument());

    fireEvent.click(screen.getByText("org/frontend"));
    expect(screen.getByLabelText("Name")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(screen.getByPlaceholderText("Search repositories...")).toBeInTheDocument();
  });

  it("does not close when the backdrop is clicked", async () => {
    const { container, props } = await renderWizard();
    const overlay = container.querySelector(".dialog-overlay");
    expect(overlay).not.toBeNull();

    fireEvent.click(overlay!);
    expect(props.onClose).not.toHaveBeenCalled();
  });

  it("resolves a preselected repository and generates a runner name", async () => {
    await renderWizard({ preselectedRepo: "org/frontend" });

    const nameInput = await screen.findByLabelText("Name");
    await waitFor(() => expect((nameInput as HTMLInputElement).value).toMatch(/^frontend-runner-/));
  });

  it("resolves a changed preselected repository while remaining mounted", async () => {
    const { rerender, props } = await renderWizard({ preselectedRepo: "org/frontend" });
    const firstName = (await screen.findByLabelText("Name")) as HTMLInputElement;
    await waitFor(() => expect(firstName.value).toMatch(/^frontend-runner-/));

    rerender(
      <AuthProvider>
        <NewRunnerWizard {...props} preselectedRepo="org/backend" />
      </AuthProvider>,
    );
    const changedName = (await screen.findByLabelText("Name")) as HTMLInputElement;
    await waitFor(() => expect(changedName.value).toMatch(/^backend-runner-/));
  });

  it("returns to repository selection when preselection is cleared", async () => {
    const { rerender, props } = await renderWizard({ preselectedRepo: "org/frontend" });
    expect(await screen.findByLabelText("Name")).toBeInTheDocument();

    rerender(
      <AuthProvider>
        <NewRunnerWizard {...props} preselectedRepo={undefined} />
      </AuthProvider>,
    );
    expect(await screen.findByPlaceholderText("Search repositories...")).toBeInTheDocument();
  });

  it("keeps a locally discovered repository that is absent from the remote list", async () => {
    vi.mocked(api.listRepos).mockResolvedValue([]);
    const localRepo = makeRepo({
      id: 0,
      full_name: "local/project",
      owner: "local",
      name: "project",
      html_url: "",
    });
    const { props } = await renderWizard({
      preselectedRepo: localRepo.full_name,
      preselectedRepoDetails: localRepo,
    });

    const nameInput = await screen.findByLabelText("Name");
    expect((nameInput as HTMLInputElement).value).toMatch(/^project-runner-/);
    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    fireEvent.click(screen.getByRole("button", { name: "Launch Runner" }));

    await waitFor(() => expect(props.onCreate).toHaveBeenCalledTimes(1));
    expect(props.onCreate).toHaveBeenCalledWith(
      expect.objectContaining({ repo_full_name: "local/project" }),
    );
  });

  it("falls back to repository selection when a preselected repository is stale", async () => {
    await renderWizard({ preselectedRepo: "org/removed" });

    expect(await screen.findByPlaceholderText("Search repositories...")).toBeInTheDocument();
    expect(screen.queryByText("Loading repository...")).not.toBeInTheDocument();
  });

  it("resolves a stale preselection when repository polling later finds it", async () => {
    vi.useFakeTimers();
    let unmount: (() => void) | undefined;
    try {
      vi.mocked(api.listRepos)
        .mockResolvedValueOnce(mockRepos.filter((repo) => repo.full_name !== "org/frontend"))
        .mockResolvedValue(mockRepos);

      const rendered = await renderWizard({ preselectedRepo: "org/frontend" });
      unmount = rendered.unmount;
      await act(async () => {
        await Promise.resolve();
      });
      expect(screen.getByPlaceholderText("Search repositories...")).toBeInTheDocument();

      await act(async () => {
        await vi.advanceTimersByTimeAsync(5000);
      });

      const nameInput = screen.getByLabelText("Name") as HTMLInputElement;
      expect(nameInput.value).toMatch(/^frontend-runner-/);
    } finally {
      unmount?.();
      vi.useRealTimers();
    }
  });

  it("Cancel button calls onClose", async () => {
    const { props } = await renderWizard();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(props.onClose).toHaveBeenCalledTimes(1);
  });

  it("shows error on failed creation", async () => {
    const onCreate = vi.fn().mockRejectedValue(new Error("Network error"));
    await renderWizard({ onCreate });
    await waitFor(() => expect(screen.getByText("org/frontend")).toBeInTheDocument());

    fireEvent.click(screen.getByText("org/frontend"));
    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    fireEvent.click(screen.getByRole("button", { name: "Launch Runner" }));

    await waitFor(() => {
      expect(screen.getByText(/Network error/)).toBeInTheDocument();
    });
  });
});
