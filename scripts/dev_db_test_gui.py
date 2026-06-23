"""
Collaborative Keystone local database test suite.

Run from VS Code or PowerShell:

    python scripts\\dev_db_test_gui.py

This GUI deliberately wraps scripts\\dev-db-test-tools.ps1 so the tested SQL and
seed logic stay in one place.
"""

from __future__ import annotations

import queue
import subprocess
import sys
import threading
import tkinter as tk
from dataclasses import dataclass
from pathlib import Path
from tkinter import messagebox, ttk
from typing import Dict, List, Optional, Tuple, Union


DEV_ACCOUNTS = (
    "test2@example.com",
    "user@example.com",
    "moderator@example.com",
)

PHASES = ("active", "closed")


@dataclass(frozen=True)
class DbCommand:
    title: str
    function_name: str
    description: str
    params: Tuple[str, ...] = ()
    caution: Optional[str] = None


QUICK_COMMANDS = (
    DbCommand(
        title="Full database reset",
        function_name="Reset-CkDatabaseFull",
        description=(
            "Clears local operational data, restores dev accounts, reseeds demo "
            "proposals, votes, merge relationships, and prints a summary."
        ),
        caution="This clears local sessions, votes, reviews, scenarios, and demo operational data.",
    ),
    DbCommand(
        title="Stage realistic environment",
        function_name="Stage-CkRealisticEnvironment",
        description=(
            "Runs a clean reset, removes DEMO prefixes from visible submission titles, "
            "adds extra realistic issues/solutions, and keeps seeded votes for feed testing. "
            "Use Full database reset before baseline sanity checks."
        ),
        caution="This resets and stages the local database for realistic browsing instead of baseline seed sanity.",
    ),
    DbCommand(
        title="Show seed summary",
        function_name="Show-CkSeedSummary",
        description="Prints dev account, board, proposal, and visible vote-count state.",
    ),
    DbCommand(
        title="Run baseline sanity checks",
        function_name="Test-CkSeedRequirements",
        description=(
            "Read-only PASS/FAIL checks for the seeded baseline against v1 rules: "
            "accounts, boards, solution target, execution fields, votes, duplicate links, trust signals, and review unlocks."
        ),
    ),
    DbCommand(
        title="Reset dev accounts",
        function_name="Reset-CkDevAccounts",
        description=(
            "Restores user@example.com, test2@example.com, and moderator@example.com. "
            "Password for all three is SuperSecurePass123."
        ),
    ),
)

USER_COMMANDS = (
    DbCommand(
        title="Reset selected user's votes and reviews",
        function_name="Reset-CkUserParticipation",
        description="Clears review actions, sentiment votes, and duplicate-link votes for the selected user.",
        params=("Email",),
    ),
    DbCommand(
        title="Reset selected user's login state",
        function_name="Reset-CkUserLoginState",
        description="Clears sessions, auth tokens, last_login_at, and keeps the selected account verified.",
        params=("Email",),
    ),
    DbCommand(
        title="Create email verification scenario",
        function_name="New-CkVerificationScenario",
        description="Marks the selected user unverified and inserts a known verification token.",
        params=("Email", "Token"),
    ),
    DbCommand(
        title="Create password reset scenario",
        function_name="New-CkPasswordResetScenario",
        description="Inserts a known password reset token for the selected user.",
        params=("Email", "Token"),
    ),
)

CYCLE_COMMANDS = (
    DbCommand(
        title="Set cycle phase",
        function_name="Set-CkCyclePhase",
        description="Moves the active cycle dates into active-month or closed state.",
        params=("Phase",),
    ),
    DbCommand(
        title="Reset moderation facet",
        function_name="Reset-CkModerationFacet",
        description="Clears appeals, reconsideration windows, watch flags, and moderator actions.",
    ),
    DbCommand(
        title="Reset trust-review facet",
        function_name="Reset-CkTrustFacet",
        description="Clears anti-abuse flags/activity signals and restores demo merge notification records.",
    ),
    DbCommand(
        title="Create trust-review scenario",
        function_name="New-CkTrustReviewScenario",
        description="Creates one open trust-review flag for moderator workflow testing.",
    ),
    DbCommand(
        title="Create appeal scenario",
        function_name="New-CkAppealScenario",
        description="Creates an archived proposal owned by test2@example.com for appeal testing.",
    ),
    DbCommand(
        title="Create reconsideration scenario",
        function_name="New-CkReconsiderationScenario",
        description="Creates an archived public proposal for moderator reconsideration testing.",
    ),
    DbCommand(
        title="Reset duplicate-link facet",
        function_name="Reset-CkMergeFacet",
        description="Clears and reseeds duplicate-link votes, relationships, notes, and reconciliations.",
    ),
    DbCommand(
        title="Create moderation hold scenario",
        function_name="New-CkModerationHoldScenario",
        description=(
            "Sets two linked water solution proposals above High Moderation-Watch: "
            "one with a 2-day-old hold timestamp and one freshly stamped for 24-hour hold testing."
        ),
    ),
    DbCommand(
        title="Reset execution tracking facet",
        function_name="Reset-CkExecutionFacet",
        description="Clears solution execution records/results and reactivates demo solution proposals.",
    ),
)


def find_repo_root() -> Path:
    start = Path(__file__).resolve()
    for candidate in (start.parent, *start.parents):
        helper = candidate / "scripts" / "dev-db-test-tools.ps1"
        if helper.exists():
            return candidate
    raise RuntimeError("Could not find scripts\\dev-db-test-tools.ps1 from this script path.")


def ps_single_quote(value: Union[str, Path]) -> str:
    return "'" + str(value).replace("'", "''") + "'"


def build_powershell_script(repo_root: Path, helper_path: Path, function_call: str) -> str:
    return (
        "$ErrorActionPreference = 'Stop'; "
        f"Set-Location -LiteralPath {ps_single_quote(repo_root)}; "
        "Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force; "
        f". {ps_single_quote(helper_path)}; "
        f"{function_call}"
    )


def run_powershell_script(ps_script: str, cwd: Path) -> int:
    process = subprocess.Popen(
        ["powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", ps_script],
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        encoding="utf-8",
        errors="replace",
    )

    assert process.stdout is not None
    for line in process.stdout:
        print(line, end="")

    return process.wait()


def run_smoke_test() -> int:
    repo_root = find_repo_root()
    helper_path = repo_root / "scripts" / "dev-db-test-tools.ps1"
    print(f"repo_root={repo_root}")
    print("command=Show-CkSeedSummary")
    ps_script = build_powershell_script(repo_root, helper_path, "Show-CkSeedSummary")
    return run_powershell_script(ps_script, repo_root)


def run_sanity_test() -> int:
    repo_root = find_repo_root()
    helper_path = repo_root / "scripts" / "dev-db-test-tools.ps1"
    print(f"repo_root={repo_root}")
    print("command=Test-CkSeedRequirements")
    ps_script = build_powershell_script(repo_root, helper_path, "Test-CkSeedRequirements")
    return run_powershell_script(ps_script, repo_root)


class DbTestSuiteApp(tk.Tk):
    def __init__(self) -> None:
        super().__init__()

        self.repo_root = find_repo_root()
        self.helper_path = self.repo_root / "scripts" / "dev-db-test-tools.ps1"
        self.output_queue = queue.Queue()
        self.current_process: Optional[subprocess.Popen] = None
        self.worker: Optional[threading.Thread] = None
        self.command_buttons: List[ttk.Button] = []

        self.title("Collaborative Keystone DB Test Suite")
        self.geometry("980x720")
        self.minsize(820, 560)

        self.email_var = tk.StringVar(value=DEV_ACCOUNTS[0])
        self.phase_var = tk.StringVar(value=PHASES[0])
        self.verification_token_var = tk.StringVar(value="dev-verify-test2")
        self.reset_token_var = tk.StringVar(value="dev-reset-test2")
        self.status_var = tk.StringVar(value="Ready")
        self.last_command_var = tk.StringVar(value="")

        self._build_ui()
        self.after(100, self._poll_output_queue)

    def _build_ui(self) -> None:
        root = ttk.Frame(self, padding=12)
        root.pack(fill=tk.BOTH, expand=True)
        root.columnconfigure(0, weight=1)
        root.rowconfigure(1, weight=1)

        header = ttk.Frame(root)
        header.grid(row=0, column=0, sticky="ew", pady=(0, 10))
        header.columnconfigure(0, weight=1)

        title = ttk.Label(header, text="Collaborative Keystone DB Test Suite", font=("Segoe UI", 15, "bold"))
        title.grid(row=0, column=0, sticky="w")

        subtitle = ttk.Label(
            header,
            text=f"Repo: {self.repo_root}",
            foreground="#555555",
        )
        subtitle.grid(row=1, column=0, sticky="w", pady=(2, 0))

        self.notebook = ttk.Notebook(root)
        self.notebook.grid(row=1, column=0, sticky="nsew")

        self._build_quick_tab()
        self._build_user_tab()
        self._build_cycle_tab()
        self._build_console_tab()

        footer = ttk.Frame(root)
        footer.grid(row=2, column=0, sticky="ew", pady=(10, 0))
        footer.columnconfigure(1, weight=1)

        ttk.Label(footer, textvariable=self.status_var).grid(row=0, column=0, sticky="w")
        ttk.Entry(footer, textvariable=self.last_command_var, state="readonly").grid(
            row=0, column=1, sticky="ew", padx=10
        )
        ttk.Button(footer, text="Copy command", command=self._copy_last_command).grid(row=0, column=2, sticky="e")

    def _build_quick_tab(self) -> None:
        tab = ttk.Frame(self.notebook, padding=12)
        tab.columnconfigure(0, weight=1)
        self.notebook.add(tab, text="Quick Reset")

        self._add_command_group(tab, "Core resets", QUICK_COMMANDS, row=0)

    def _build_user_tab(self) -> None:
        tab = ttk.Frame(self.notebook, padding=12)
        tab.columnconfigure(0, weight=1)
        self.notebook.add(tab, text="User State")

        controls = ttk.LabelFrame(tab, text="Selected user", padding=10)
        controls.grid(row=0, column=0, sticky="ew", pady=(0, 10))
        controls.columnconfigure(1, weight=1)

        ttk.Label(controls, text="Email").grid(row=0, column=0, sticky="w", padx=(0, 8))
        email_box = ttk.Combobox(controls, textvariable=self.email_var, values=DEV_ACCOUNTS)
        email_box.grid(row=0, column=1, sticky="ew")

        ttk.Label(controls, text="Verification token").grid(row=1, column=0, sticky="w", padx=(0, 8), pady=(8, 0))
        ttk.Entry(controls, textvariable=self.verification_token_var).grid(row=1, column=1, sticky="ew", pady=(8, 0))

        ttk.Label(controls, text="Password reset token").grid(row=2, column=0, sticky="w", padx=(0, 8), pady=(8, 0))
        ttk.Entry(controls, textvariable=self.reset_token_var).grid(row=2, column=1, sticky="ew", pady=(8, 0))

        self._add_command_group(tab, "Account and participation scenarios", USER_COMMANDS, row=1)

    def _build_cycle_tab(self) -> None:
        tab = ttk.Frame(self.notebook, padding=12)
        tab.columnconfigure(0, weight=1)
        self.notebook.add(tab, text="Cycle & Facets")

        controls = ttk.LabelFrame(tab, text="Cycle phase", padding=10)
        controls.grid(row=0, column=0, sticky="ew", pady=(0, 10))
        controls.columnconfigure(1, weight=1)

        ttk.Label(controls, text="Phase").grid(row=0, column=0, sticky="w", padx=(0, 8))
        phase_box = ttk.Combobox(controls, textvariable=self.phase_var, values=PHASES, state="readonly")
        phase_box.grid(row=0, column=1, sticky="ew")

        self._add_command_group(tab, "Cycle, moderation, duplicate links, and execution", CYCLE_COMMANDS, row=1)

    def _build_console_tab(self) -> None:
        tab = ttk.Frame(self.notebook, padding=12)
        tab.columnconfigure(0, weight=1)
        tab.rowconfigure(1, weight=1)
        self.notebook.add(tab, text="Console")

        toolbar = ttk.Frame(tab)
        toolbar.grid(row=0, column=0, sticky="ew", pady=(0, 8))
        ttk.Button(toolbar, text="Clear console", command=self._clear_console).pack(side=tk.LEFT)
        ttk.Button(toolbar, text="Show summary", command=lambda: self._run_command(QUICK_COMMANDS[1])).pack(
            side=tk.LEFT, padx=(8, 0)
        )

        console_frame = ttk.Frame(tab)
        console_frame.grid(row=1, column=0, sticky="nsew")
        console_frame.columnconfigure(0, weight=1)
        console_frame.rowconfigure(0, weight=1)

        self.console = tk.Text(console_frame, wrap=tk.WORD, height=18, undo=False)
        self.console.grid(row=0, column=0, sticky="nsew")
        scrollbar = ttk.Scrollbar(console_frame, command=self.console.yview)
        scrollbar.grid(row=0, column=1, sticky="ns")
        self.console.configure(yscrollcommand=scrollbar.set)

        self._append_console("Ready. Commands run through scripts\\dev-db-test-tools.ps1.\n")

    def _add_command_group(
        self,
        parent: ttk.Frame,
        label: str,
        commands: Tuple[DbCommand, ...],
        row: int,
    ) -> None:
        group = ttk.LabelFrame(parent, text=label, padding=10)
        group.grid(row=row, column=0, sticky="ew", pady=(0, 10))
        group.columnconfigure(0, weight=1)

        for index, command in enumerate(commands):
            card = ttk.Frame(group, padding=(0, 6))
            card.grid(row=index * 2, column=0, sticky="ew")
            card.columnconfigure(0, weight=1)

            ttk.Label(card, text=command.title, font=("Segoe UI", 10, "bold")).grid(row=0, column=0, sticky="w")
            ttk.Label(card, text=command.description, foreground="#555555", wraplength=670).grid(
                row=1, column=0, sticky="w", pady=(2, 0)
            )

            button = ttk.Button(card, text="Run", command=lambda cmd=command: self._run_command(cmd))
            button.grid(row=0, column=1, rowspan=2, sticky="e", padx=(12, 0))
            self.command_buttons.append(button)

            if index < len(commands) - 1:
                separator = ttk.Separator(group)
                separator.grid(row=(index * 2) + 1, column=0, sticky="ew", pady=4)

    def _params_for(self, command: DbCommand) -> Dict[str, str]:
        params: Dict[str, str] = {}
        for param in command.params:
            if param == "Email":
                params[param] = self.email_var.get().strip()
            elif param == "Phase":
                params[param] = self.phase_var.get().strip()
            elif param == "Token" and command.function_name == "New-CkVerificationScenario":
                params[param] = self.verification_token_var.get().strip()
            elif param == "Token" and command.function_name == "New-CkPasswordResetScenario":
                params[param] = self.reset_token_var.get().strip()
        return params

    def _validate_params(self, command: DbCommand, params: Dict[str, str]) -> bool:
        missing = [name for name, value in params.items() if not value]
        if missing:
            messagebox.showwarning("Missing value", f"Enter a value for: {', '.join(missing)}")
            return False

        phase = params.get("Phase")
        if phase and phase not in PHASES:
            messagebox.showwarning("Invalid phase", f"Choose one of: {', '.join(PHASES)}")
            return False

        return True

    def _build_function_call(self, command: DbCommand, params: Dict[str, str]) -> str:
        pieces = [command.function_name]
        for name, value in params.items():
            pieces.append(f"-{name}")
            pieces.append(ps_single_quote(value))
        return " ".join(pieces)

    def _build_powershell_script(self, function_call: str) -> str:
        return build_powershell_script(self.repo_root, self.helper_path, function_call)

    def _run_command(self, command: DbCommand) -> None:
        if self.current_process is not None:
            messagebox.showinfo("Command running", "Wait for the current command to finish first.")
            return

        if command.caution:
            confirmed = messagebox.askyesno("Confirm reset", f"{command.caution}\n\nRun it now?")
            if not confirmed:
                return

        params = self._params_for(command)
        if not self._validate_params(command, params):
            return

        function_call = self._build_function_call(command, params)
        ps_script = self._build_powershell_script(function_call)
        self.last_command_var.set(function_call)
        self.status_var.set(f"Running {command.title}...")
        self._set_buttons_enabled(False)
        self.notebook.select(self.notebook.tabs()[-1])
        self._append_console(f"\n> {function_call}\n")

        self.worker = threading.Thread(
            target=self._run_powershell_worker,
            args=(ps_script,),
            daemon=True,
        )
        self.worker.start()

    def _run_powershell_worker(self, ps_script: str) -> None:
        try:
            process = subprocess.Popen(
                ["powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", ps_script],
                cwd=self.repo_root,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                encoding="utf-8",
                errors="replace",
            )
            self.current_process = process

            assert process.stdout is not None
            for line in process.stdout:
                self.output_queue.put(("line", line))

            exit_code = process.wait()
            self.output_queue.put(("done", exit_code))
        except FileNotFoundError:
            self.output_queue.put(("line", "Could not find powershell.exe. Run this on Windows PowerShell.\n"))
            self.output_queue.put(("done", 1))
        except Exception as exc:  # noqa: BLE001 - surfaces unexpected local tool failures to the GUI.
            self.output_queue.put(("line", f"{exc}\n"))
            self.output_queue.put(("done", 1))

    def _poll_output_queue(self) -> None:
        try:
            while True:
                kind, payload = self.output_queue.get_nowait()
                if kind == "line":
                    self._append_console(str(payload))
                elif kind == "done":
                    exit_code = int(payload)
                    self.current_process = None
                    self.worker = None
                    self._set_buttons_enabled(True)
                    if exit_code == 0:
                        self.status_var.set("Done")
                        self._append_console("Command completed.\n")
                    else:
                        self.status_var.set(f"Command failed with exit code {exit_code}")
                        self._append_console(f"Command failed with exit code {exit_code}.\n")
        except queue.Empty:
            pass

        self.after(100, self._poll_output_queue)

    def _append_console(self, text: str) -> None:
        self.console.insert(tk.END, text)
        self.console.see(tk.END)

    def _clear_console(self) -> None:
        self.console.delete("1.0", tk.END)

    def _set_buttons_enabled(self, enabled: bool) -> None:
        state = tk.NORMAL if enabled else tk.DISABLED
        for button in self.command_buttons:
            button.configure(state=state)

    def _copy_last_command(self) -> None:
        command = self.last_command_var.get()
        if not command:
            return
        self.clipboard_clear()
        self.clipboard_append(command)
        self.status_var.set("Copied command")


def main() -> None:
    if "--smoke" in sys.argv:
        raise SystemExit(run_smoke_test())

    if "--sanity" in sys.argv:
        raise SystemExit(run_sanity_test())

    app = DbTestSuiteApp()
    app.mainloop()


if __name__ == "__main__":
    main()
