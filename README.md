# Saldo

A desktop app for a holistic overview of personal or family finances. Track balances across all your accounts — bank accounts, savings, investments, cash, crypto — in one place, record cash flows and transfers when you need more detail, and see your total net worth at a glance.

**Low-maintenance by design.** You do not need to log every transaction. Just updating balances occasionally is enough to keep a reliable snapshot history. When you want more detail, Saldo also supports cash flows, linked transfers between your own accounts, partner records, CSV and eKasa import, buckets, assets and a tax forecast.

Built with React + TypeScript + Vite (frontend) and Tauri + Rust + SQLite (backend). Data is stored locally on your machine — no cloud, no account required. The database location is configurable, so it can live in a cloud-backed-up folder while the app remains offline-first.

## Features

### Balances and the dashboard

- Live dashboard of every account, including entries your bank dated after today
- Time machine: pick a date to see balances as they were
- Net Worth, Liquid, Total Balance and Left to Spend, each with a breakdown of the lines behind it
- Warnings for accounts with no exchange rate or an old one
- Multi-currency support with FX rates, and custom units with prices you type
- Bulk balance update as of any date

### Transactions and partners

- **Cash flows** capture money moving in or out of an account without full double-entry bookkeeping.
- **Transfers** link two of your own accounts so balances stay consistent on both sides.
- **Partners** represent counterparties and can store IBANs for matching imported transactions.
- **CSV import** brings in bank exports with saved profiles per bank, duplicate detection and partner matching.
- A ledger with filters for every kind of entry

### Buckets

- Capital buckets (Emergency Fund, Vacation, ...) with account-linked allocations
- A switch on every bucket card that keeps it out of Left to Spend for a while
- Income budgets for income that changes every month: type a budget per month, and Left to Spend shows budgeted income minus spending

### Persons, companies and taxes

- Give accounts an owner and filter the dashboard and the ledger by person
- Revenue, expenses and prepaid expenses with VAT, deductible share and private use, linked to the payments that paid them
- eKasa receipt import from a QR code or PDF, with rules that set each item's shares
- Assets with depreciation, VAT reclaimed and a business-use share that can change from a given month
- Tax models that forecast the whole year: corporate or personal income tax, minimum tax, reserve fund, dividends and profit left from earlier years

### Everything else

- Claude can record invoices and receipts for you (see below)
- Dark and light theme
- English and Slovak
- Demo mode to explore the app without entering real data
- Custom database file location
- Offline-first, single-user design

## Installation

Download the latest `saldo_x.x.x_aarch64.dmg` from the [Releases](../../releases) page, open it and drag **saldo** into Applications.

- The macOS build is for Apple Silicon only.
- It is not signed with an Apple Developer ID. If macOS refuses to open it, go to System Settings → Privacy & Security and click **Open Anyway**.
- Releases up to 0.3.0 shipped a Windows installer. For a newer version on Windows, build from source (below).

## Recording invoices with Claude

The installed app doubles as an MCP server: `saldo mcp` serves Saldo's tools to Claude instead of opening the window. Claude reads your invoices, credit notes and eKasa receipts, shows you one review table per batch, and records them only after you approve. One step undoes a batch.

Register the server once with [Claude Code](https://claude.com/claude-code):

```bash
claude mcp add --scope user saldo -- /Applications/saldo.app/Contents/MacOS/saldo mcp
```

The `record-invoices` skill in `.claude/skills/` tells Claude how to read the documents and use the tools. When the Saldo window comes back to the front after Claude wrote, it reloads its data.

## Running from source

### Prerequisites

- [Node.js](https://nodejs.org/) (LTS)
- [pnpm](https://pnpm.io/) (`npm install -g pnpm`)
- [Rust](https://rustup.rs/) (stable toolchain)
- macOS: Xcode Command Line Tools (`xcode-select --install`)
- Windows: Visual Studio C++ Build Tools and the WebView2 Runtime (pre-installed on Windows 10/11)
- For eKasa PDF import: the PDFium library for your platform from [pdfium-binaries](https://github.com/bblanchon/pdfium-binaries), placed in `src-tauri/` as `libpdfium.dylib` (macOS) or `pdfium.dll` (Windows)

### Setup

```bash
pnpm install
```

### Development

```bash
# Start Tauri dev (frontend + backend) on your real saldo.db
pnpm tauri dev

# Same, but with a separate development database
pnpm tauri dev --config src-tauri/tauri.dev.conf.json

# Start frontend only (Vite); calls to the backend fail without Tauri
pnpm dev
```

### Testing

```bash
# TypeScript unit tests
pnpm test

# Rust unit tests
cd src-tauri && cargo test

# Everything CI runs: format check, lint, Vitest, cargo fmt, clippy, cargo test
pnpm run ci

# E2E tests (Playwright), with `pnpm tauri dev` already running
pnpm test:e2e
```

### Linting & Formatting

```bash
pnpm lint
pnpm format
cd src-tauri && cargo fmt && cargo clippy -- -D warnings
```

### Building

```bash
pnpm tauri build
```

## Project Structure

- `src/` — React + TypeScript frontend
  - `app/` — app shell, modal wiring, and top-level navigation
  - `features/` — domain features such as dashboard, ledger, transactions, partners, buckets, assets, tax models, eKasa and settings
  - `shared/` — reusable UI components, types, config, and Tauri IPC wrappers
  - `i18n/` — translations (English, Slovak)
  - `styles/` — global styling
- `src-tauri/` — Rust backend
  - `src/features/` — Tauri commands and SQLite access per domain
  - `src/mcp/` — the `saldo mcp` server
- `migrations/` — SQL migration files
- `schema/` — Reference DDL and seed documentation
- `tests/` — E2E tests (Playwright)
- `.claude/skills/` — skills for Claude: recording invoices, verifying changes in the running app
