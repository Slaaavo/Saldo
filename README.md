# Saldo

A desktop app for a holistic overview of personal or family finances. Track balances across all your accounts — bank accounts, savings, investments, cash, crypto — in one place and see your total net worth at a glance.

**Low-maintenance by design.** You don't need to log every transaction. Just update the balances as often as is relevant for you — once a month is plenty. Saldo shows you where you stand on any given date by storing a history of balance snapshots.

Built with React + TypeScript + Vite (frontend) and Tauri + Rust + SQLite (backend). Data is stored locally on your machine — no cloud, no account required. Database location is configurable, so can be put in a cloud backed up folder,and the app is offline-first.

## Features

- Dashboard with date-based balance snapshots across all accounts
- Aggregated total balance and "left to spend" calculation
- Capital buckets (e.g. Emergency Fund, Vacation) with account-linked allocations
- Multi-currency support with FX rates
- Create, edit, and delete balance update events
- Ledger of balance history per account
- Bulk balance update
- Dark and light theme
- Multiple languages (English, Slovak)
- Demo mode to explore the app without entering real data
- Custom database file location
- Offline-first, single-user design

## Installation

Download the latest `Saldo_x.x.x_x64-setup.exe` installer from the [Releases](../../releases) page and run it. The installer will guide you through the setup. No additional dependencies are required — WebView2 is bundled with Windows 10/11.

After installation, launch **Saldo** from the Start Menu or Desktop shortcut.

# Running from source

## Prerequisites

- [Node.js](https://nodejs.org/) (LTS)
- [pnpm](https://pnpm.io/) (`npm install -g pnpm`)
- [Rust](https://rustup.rs/) (stable toolchain)
- Visual Studio C++ Build Tools (Windows)
- WebView2 Runtime (pre-installed on Windows 10/11)

## Setup

```bash
pnpm install
```

## Development

```bash
# Start Tauri dev (frontend + backend)
pnpm tauri dev

# Start frontend only (Vite)
pnpm dev
```

## Testing

```bash
# TypeScript unit tests
pnpm test

# Rust unit tests
cd src-tauri && cargo test

# Lint & format check
pnpm ci
```

## Linting & Formatting

```bash
pnpm lint
pnpm format
cd src-tauri && cargo fmt && cargo clippy
```

## Building

```bash
pnpm tauri build
```

## Project Structure

- `src/` — React + TypeScript frontend
  - `pages/` — Page-level views (Dashboard, Settings, FX Rates)
  - `components/` — Reusable UI components and modals
  - `hooks/` — Custom React hooks
  - `api/` — Tauri IPC command wrappers
  - `types/` — TypeScript type definitions
  - `i18n/` — Translations (English, Slovak)
- `src-tauri/` — Rust backend (Tauri commands, SQLite access)
- `migrations/` — SQL migration files
- `schema/` — Reference DDL and seed documentation
- `tests/` — E2E tests (Playwright)
