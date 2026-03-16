# Saldo

A desktop app for a holistic overview of personal or family finances. Track balances across all your accounts — bank accounts, savings, investments, cash, crypto — in one place, record cash flows and transfers when you need more detail, and see your total net worth at a glance.

**Low-maintenance by design.** You do not need to log every transaction. Just updating balances occasionally is enough to keep a reliable snapshot history. When you want more detail, Saldo also supports cash flows, linked transfers between your own accounts, partner records, and CSV import for bank exports.

Built with React + TypeScript + Vite (frontend) and Tauri + Rust + SQLite (backend). Data is stored locally on your machine — no cloud, no account required. The database location is configurable, so it can live in a cloud-backed-up folder while the app remains offline-first.

## Features

- Dashboard with date-based balance snapshots across all accounts
- Cash flows for income and expense tracking alongside balance snapshots
- Linked transfers between your own accounts
- Partner directory with optional IBAN management for counterparties
- CSV import for cash flows and transfers with account/partner matching
- Aggregated total balance and "left to spend" calculation
- Capital buckets (e.g. Emergency Fund, Vacation) with account-linked allocations
- Multi-currency support with FX rates
- Create, edit, and delete balance updates, cash flows, and transfers
- Ledger with filters for balance updates, cash flows, and transfers
- Bulk balance update
- Dark and light theme
- Multiple languages (English, Slovak)
- Demo mode to explore the app without entering real data
- Custom database file location
- Offline-first, single-user design

## Transactions and Partners

Saldo still works well as a snapshot-based finance app, but it now also supports more detailed cash movement tracking:

- **Cash flows** capture money moving in or out of an account without requiring full double-entry bookkeeping.
- **Transfers** create linked events between two of your own accounts so balances stay consistent on both sides.
- **Partners** represent external counterparties and can store IBANs for matching imported transactions.
- **CSV import** helps bring in bank exports, detect duplicates, match own accounts vs partners, and create new partners during import when needed.

## Installation

Download the latest `Saldo_x.x.x_x64-setup.exe` installer from the [Releases](../../releases) page and run it. The installer will guide you through the setup. No additional dependencies are required — WebView2 is bundled with Windows 10/11.

After installation, launch **Saldo** from the Start Menu or Desktop shortcut.

## Running from source

### Prerequisites

- [Node.js](https://nodejs.org/) (LTS)
- [pnpm](https://pnpm.io/) (`npm install -g pnpm`)
- [Rust](https://rustup.rs/) (stable toolchain)
- Visual Studio C++ Build Tools (Windows)
- WebView2 Runtime (pre-installed on Windows 10/11)

### Setup

```bash
pnpm install
```

### Development

```bash
# Start Tauri dev (frontend + backend)
pnpm tauri dev

# Start frontend only (Vite)
pnpm dev
```

### Testing

```bash
# TypeScript unit tests
pnpm test

# Rust unit tests
cd src-tauri && cargo test

# Lint & format check
pnpm ci
```

### Linting & Formatting

```bash
pnpm lint
pnpm format
cd src-tauri && cargo fmt && cargo clippy
```

### Building

```bash
pnpm tauri build
```

## Project Structure

- `src/` — React + TypeScript frontend
  - `app/` — app shell, modal wiring, and top-level navigation
  - `features/` — domain features such as dashboard, ledger, transactions, partners, settings, assets, and currency
  - `shared/` — reusable UI components, types, config, and Tauri IPC wrappers
  - `i18n/` — translations (English, Slovak)
  - `styles/` — global styling
- `src-tauri/` — Rust backend (Tauri commands, SQLite access)
- `migrations/` — SQL migration files
- `schema/` — Reference DDL and seed documentation
- `tests/` — E2E tests (Playwright)
