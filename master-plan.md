# Master Plan — Personal Finance Desktop App

Date: 2026-03-02

This document captures a concise, high-level plan to build a small desktop app for tracking personal finances. The goal is an approachable MVP so you can experiment and learn modern, LLM-friendly tooling.

1. Define Project Goals & Scope - done in `goals-and-scope.md` 

2. Choose Tech Stack
   - Prefer modern, widely-known tools: TypeScript + React (or Solid/Vue) for UI, Tauri (or Electron) for desktop shell, and SQLite for local storage. Add tooling: ESLint, Prettier, Vitest/Jest.

3. Design Data Model / DB Schema
   - Define entities: Account, Transaction, Category, Budget, Tag, UserSettings; relationships and indices for queries; migration strategy and JSON/CSV export format.

4. Write Use Cases & User Stories
   - Capture main flows (add/edit transaction, categorize, monthly report, reconcile, backup/restore), acceptance criteria, and common error states.

5. Design Screens & User Flows
   - Sketch the UI: Dashboard, Accounts list, Transaction editor, Budgets, Reports, Settings. Map navigation, keyboard shortcuts, and minimal modal patterns.

6. Architecture & Component Breakdown
   - Define renderer vs native responsibilities (UI vs DB/files), state management pattern (unidirectional store or MVVM), IPC boundaries, and core services (import/export, backup, categorization).

7. Set Up Dev Environment & Tooling
   - Initialize repository, TypeScript, lint/format, test runner, and basic CI for lint/tests. Add scripts for local dev, build, and packaging.

8. Implement MVP Features
   - Build core flows: DB init, add/edit/delete transactions, list/search, simple dashboard totals, CSV/JSON import and export. Iterate fast with small commits.

9. Add Persistence, Sync & Backup
   - Provide robust local backups and an encrypted export/import workflow; plan optional encrypted cloud sync later (Dropbox/Google Drive hooks or user-provided storage).

10. Testing, CI, and Packaging
    - Add unit tests for business logic, lightweight E2E/smoke tests for critical flows, and configure packaging (Tauri/Electron) for native installers.

11. Polish UI/UX and Accessibility
    - Improve visuals, theming (dark/light), keyboard navigation, ARIA labels, and small micro-interactions to make the app delightful while remaining minimal.

12. Iterate, Add Features, Document
    - Triage improvements, add advanced features (recurring transactions, budgets, charts, ML-assisted categorization), keep README and architecture notes up to date.

Next steps: pick a tech stack and draft the DB schema so we can start the repo scaffolding.
