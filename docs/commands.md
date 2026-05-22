# Commands Reference

## Development
- `bun install` - Install dependencies
- `bun dev:mock` - Frontend-only development with mock data (recommended for UI work)
- `bun tauri dev` - Full Tauri app with Rust backend (for end-to-end testing)
- `bun tauri build` - Production build

## Testing
See [TESTING.md](/TESTING.md) for full documentation.

Quick reference:
- `bun run test:all` - Run all tests (unit, service, Rust, integration)
- `bun run test:unit` - Run TypeScript unit tests (files: `*.unit.test.ts`)
- `bun run test:service` - Run service layer tests (files: `*.service.test.ts`)
- `bun run test:rust:unit` - Run Rust unit tests (`cargo test --lib`)
- `bun run test:integration` - Run database integration tests (requires Docker)
  - `IT_DB=mysql bun run test:integration` - Test specific database
  - `IT_DB=all bun run test:integration` - Test all databases
  - `IT_REUSE_LOCAL_DB=1` - Reuse existing local database containers
- `bun run test:smoke` - Quick validation (typecheck, lint, unit tests)
- `bun run test:ci` - Full CI test suite

## Code Quality
- `bun run typecheck` - TypeScript type checking
- `bun run lint` - Lint TypeScript/JSON files with Prettier
- `bun run lint:rust` - Check Rust code (`cargo check`)
- `bun run format` - Format TypeScript files with Prettier

## Website
- `bun run website:dev` - Run Astro marketing site locally
- `bun run website:build` - Build marketing site
