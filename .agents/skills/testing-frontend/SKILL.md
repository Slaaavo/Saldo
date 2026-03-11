---
name: testing-frontend
description: 'Write and run unit tests for React components and TypeScript utilities using Vitest and React Testing Library. Use when asked to "write tests", "add test coverage", "create component tests", "test this file", "make it safe to refactor", or when testing React pages, hooks, or utility functions. Covers pure function testing, component rendering, user interaction simulation, and API mocking patterns for Tauri apps.'
---

# Frontend Testing Skill

Write effective unit and component tests for the React + TypeScript frontend (`src/`) using Vitest and React Testing Library.

## Scope

This skill covers **frontend code only** — files under `src/`:
- React components (`src/components/`, `src/pages/`)
- Custom hooks (`src/hooks/`)
- TypeScript utilities (`src/utils/`)
- Type definitions used by the UI (`src/types/`)

**Out of scope:** Rust backend code in `src-tauri/` (tested separately with `cargo test` and Clippy). Playwright e2e specs in `tests/` are also out of scope — they have their own runner.

## When to Use This Skill

- User asks to write tests for a file under `src/`
- User asks to test a React component, page, hook, or utility function
- User wants to add test coverage before refactoring frontend code
- User asks to "make it safe to refactor" a `.tsx` or `.ts` file

## Tech Stack

- **Test runner:** Vitest (v4+), configured in `vite.config.ts`
- **Component testing:** `@testing-library/react`, `@testing-library/jest-dom`, `@testing-library/user-event`
- **DOM environment:** jsdom (configured globally)
- **Setup file:** `src/test-setup.ts` (imports `@testing-library/jest-dom`)
- **Globals:** enabled (`describe`, `it`, `expect` available without import — but prefer explicit imports for clarity)

## Strategy: Test from the Inside Out

When covering a file with tests, follow this priority:

1. **Extract pure functions first** — move logic with no React/state dependencies into `src/utils/`. These are the easiest to test and most valuable to protect.
2. **Test pure functions with Vitest** — no mocking needed, fast, deterministic.
3. **Extract derived data computations** — if a component computes data from state (filtering, sorting, pivoting), extract it to a testable function.
4. **Component tests last** — mock the API layer and i18n, render the component, assert on behavior.

## File Conventions

| Source file | Test file |
|------------|-----------|
| `src/utils/foo.ts` | `src/utils/foo.test.ts` |
| `src/pages/FooPage.tsx` | `src/pages/FooPage.test.tsx` |
| `src/hooks/useFoo.ts` | `src/hooks/useFoo.test.ts` |
| `src/components/Foo.tsx` | `src/components/Foo.test.tsx` |

## Running Frontend Tests

```bash
# Run all frontend unit tests (always scope to src/ to exclude Playwright e2e specs)
npx vitest run src/

# Run a specific test file
npx vitest run src/utils/fxRate.test.ts

# Run tests in watch mode
npx vitest src/
```

> **Important:** Always pass `src/` to Vitest. Running bare `npx vitest run` will pick up Playwright specs in `tests/` which fail under Vitest.

## Pattern 1: Testing Pure Utility Functions

Pure functions are the best candidates for testing. Extract them to `src/utils/` and test directly.

```ts
// src/utils/foo.test.ts
import { describe, it, expect } from 'vitest';
import { myFunction } from './foo';

describe('myFunction', () => {
  it('handles normal input', () => {
    expect(myFunction('input')).toBe('expected');
  });

  it('returns null for invalid input', () => {
    expect(myFunction('')).toBeNull();
  });
});
```

### Guidelines for utility tests:
- Test happy path, edge cases, and error cases
- Add roundtrip tests when there are paired encode/decode functions
- No mocking needed — these are pure

## Pattern 2: Testing React Components

Components require mocking the API layer (Tauri IPC), i18n, and sometimes complex sub-components.

### Required mocks

**API layer** — All API calls go through `src/api/index.ts` which wraps Tauri `invoke`. Always mock the whole module:

```ts
vi.mock('../api', () => ({
  listFxRates: vi.fn(),
  fetchFxRates: vi.fn(),
  // ... all functions the component imports
}));
```

**i18n** — Use a pass-through mock that returns the translation key:

```ts
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => {
      if (opts) {
        let result = key;
        for (const [k, v] of Object.entries(opts)) {
          result = result.replace(`{{${k}}}`, String(v));
        }
        return result;
      }
      return key;
    },
    i18n: { language: 'en' },
  }),
}));
```

**Complex sub-components** — Mock components like DatePicker that depend on Popover/Calendar:

```ts
vi.mock('../components/ui/date-picker', () => ({
  DatePicker: ({ value, onChange }: { value: string; onChange: (v: string) => void }) => (
    <input data-testid="date-picker" value={value} onChange={(e) => onChange(e.target.value)} />
  ),
}));
```

### Component test structure

```tsx
import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import MyPage from './MyPage';

// ... mocks above ...

import { myApiCall } from '../api';

function setupMocks(overrides?: { /* typed overrides */ }) {
  (myApiCall as Mock).mockResolvedValue(defaultData);
  // ... setup all mocks with defaults, apply overrides
}

describe('MyPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders initial state', async () => {
    setupMocks();
    render(<MyPage />);
    await waitFor(() => {
      expect(screen.getByText('expected text')).toBeInTheDocument();
    });
  });

  it('handles user interaction', async () => {
    setupMocks();
    const user = userEvent.setup();
    render(<MyPage />);

    await waitFor(() => {
      expect(screen.getByText('button label')).toBeInTheDocument();
    });

    await user.click(screen.getByText('button label'));

    await waitFor(() => {
      expect(myApiCall).toHaveBeenCalledWith(expectedArgs);
    });
  });
});
```

### What to test in components:

| Category | Examples |
|----------|---------|
| **Mount behavior** | API calls on init, data displayed correctly |
| **Empty/loading states** | Empty message, loading indicator |
| **Error handling** | Error banner when API fails |
| **User actions** | Button clicks trigger correct API calls |
| **Conditional rendering** | Elements appear/disappear based on data |
| **Form interactions** | Input → type → submit → API called with correct args |
| **Keyboard interactions** | Enter to save, Escape to cancel |

### Common pitfalls:

- **`act(...)` warnings in stderr** — These are benign when caused by async state updates during mount. They don't affect test correctness.
- **Async data loading** — Always wrap assertions for loaded data in `waitFor(() => { ... })`.
- **Mock setup order** — Place `vi.mock()` calls BEFORE the import of the mocked module (Vitest hoists them, but keep it readable).
- **`setupMocks` helper** — Always create one. It provides defaults and accepts overrides, keeping individual tests focused on what they're changing.
- **`beforeEach(() => vi.clearAllMocks())`** — Always include this to prevent test pollution.

## Pattern 3: Testing Custom Hooks

Use `renderHook` from `@testing-library/react`:

```ts
import { renderHook, waitFor } from '@testing-library/react';
import { useFxRates } from './useFxRates';

// ... same API mocks as component tests ...

it('loads rates on mount', async () => {
  setupMocks();
  const { result } = renderHook(() => useFxRates());

  await waitFor(() => {
    expect(result.current.dates).toHaveLength(2);
  });
});
```

## Refactoring Safety Checklist

Before refactoring a file:

1. Check if tests exist (search for `*.test.ts` / `*.test.tsx` alongside the file)
2. If no tests exist:
   - Extract pure functions → write unit tests
   - Extract derived computations → write unit tests
   - Write component tests for key user flows
3. Run tests: `npx vitest run src/`
4. Refactor with confidence
5. Run tests again to verify
