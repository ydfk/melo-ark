# Agent Guide

This file tells coding agents how to build, test, and follow project conventions in this repo.

## Repository summary

- Stack: React 19.2 + Vite 8.1 + TypeScript 7 + Tailwind v4
- Router: react-router-dom
- UI: shadcn/ui + Radix UI
- State: zustand
- Forms: react-hook-form + zod
- API: Alova with contract-compatible mock and real adapters
- Formatting: oxfmt
- Linting: oxlint
- Tests: vitest + Testing Library (jsdom)

## Commands

All commands use pnpm (see `package.json`).

### Install

- `pnpm install`

### Dev server

- `pnpm dev`

### Build

- `pnpm build` (runs `tsc -b` then `vite build`)

### Lint

- `pnpm lint`

### Format

- `pnpm format`
- `pnpm format:check`

### Tests (Vitest)

- `pnpm test` (watch mode)
- `pnpm test:run` (CI-style run)

#### Run a single test

- By file: `pnpm test -- src/path/to/file.test.tsx`
- By name: `pnpm test -- -t "test name"`
- By pattern: `pnpm test -- shimmer-button`

## Test environment

- Runner: Vitest
- Environment: jsdom
- Global test APIs enabled (`globals: true`)
- Setup file: `src/test/setup.ts`
  - Includes `@testing-library/jest-dom/vitest`
  - Stubs `window.matchMedia`
- Type globals: `vitest/globals` and `@testing-library/jest-dom` are enabled in `tsconfig.app.json`
- Vitest excludes `.worktrees` from test discovery

## Code style guidelines

### Formatting

- Use oxfmt with config in `.oxfmtrc.json`
  - `tabWidth: 2`
  - `semi: true`
  - `singleQuote: false`
  - `trailingComma: "es5"`
- Prefer double quotes in TS/TSX
- Keep lines concise and readable; avoid deep nesting

### Imports

- Use path alias `@/` for `src` imports (see `tsconfig.json`)
- Group imports in this order:
  1. React/third-party
  2. Absolute project imports (`@/`)
  3. Relative imports
- Avoid unused imports; oxlint will flag them

### TypeScript

- Strict mode is enabled (`strict: true`)
- Avoid `any`; prefer precise types and inferred types
- Use `React.ComponentPropsWithoutRef<"button">` when you want DOM props only
- Use `HTMLMotionProps<"button">` if motion props are required
- Prefer named exports for reusable components

### React components

- Keep components focused and small
- Prefer function components
- Use `className` + `cn` utility for styling
- Keep props minimal and predictable
- Use `aria-*` attributes and proper semantic elements

### Styling (Tailwind)

- Favor Tailwind utility classes over custom CSS
- Keep class lists readable (group related utilities)
- Avoid inline styles unless necessary (animations or dynamic values)
- Respect existing shadcn patterns

### Files and structure

- UI components: `src/components/ui/*`
- Magic UI components: `src/components/magicui/*`
- Demo pages: `src/pages/components/*`
- Layout: `src/components/layout/*`
- Tests co-located with the components/pages they cover

### Naming

- Components: PascalCase (`MagicUiDemo`)
- Files: kebab-case for pages/components (e.g. `magicui-demo.tsx`)
- Tests: `*.test.tsx`
- Hooks: `useXxx`

### Error handling

- Prefer user-facing messages in UI (e.g. toast or inline)
- Validate inputs early (forms + zod)
- Avoid silent failures; surface errors where feasible

### Accessibility

- Use semantic HTML elements
- Ensure focus-visible styles exist for interactive elements
- Provide labels for inputs and buttons
- Avoid motion for users who prefer reduced motion

## Linting rules (inferred)

- Unused locals/params are errors (`noUnusedLocals`, `noUnusedParameters`)
- No fallthrough in switch (`noFallthroughCasesInSwitch`)
- Avoid unchecked side-effect imports (`noUncheckedSideEffectImports`)

## Project specifics to keep in mind

- Vite is on stable 8.1.5. Avoid downgrading unless required.
- Recharts 2.15.4 is an intentional compatibility pin for the current shadcn chart source; verify the chart against Recharts 3 before removing it.
- `pnpm` 11.17 is the package manager. Don’t use npm/yarn.
- Preserve `CONTRACT.md` paths, payloads, statuses, Bearer assignment, and Problem details.
- Use `VITE_USE_MOCK` for adapter selection and `VITE_API_BASE_URL` or `VITE_PROXY_HOST` for backend selection.
- Windows line endings may appear; avoid reformatting unrelated files.
- `.worktrees` is ignored in git and excluded from tests.

## Existing agent rules

- No Cursor rules found in `.cursor/rules/` or `.cursorrules`.
- No Copilot rules found in `.github/copilot-instructions.md`.

## When unsure

- Prefer minimal, isolated changes
- Follow existing patterns in nearby files
- Update tests when behavior changes
