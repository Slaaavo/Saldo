# Saldo

A personal finance desktop app for tracking account balances. Built with React + TypeScript + Vite (frontend) and Tauri + Rust + SQLite (backend).

## Features

- Dashboard showing per-account balances for a selected date
- Aggregated total balance
- Create, edit, and delete balance update events
- Multiple account support
- Date-based snapshot navigation
- Offline-first, single-user design

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
- `src-tauri/` — Rust backend (Tauri commands, SQLite access)
- `migrations/` — SQL migration files
- `schema/` — Reference DDL and seed documentation
- `plans/` — Implementation plans
- `tests/` — E2E tests (Playwright, planned)
