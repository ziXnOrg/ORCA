# Rule: TypeScript Standards (SDK/Tools)

- TSConfig: `strict: true`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`.
- Lint: ESLint with `@typescript-eslint` (errors); Format: Prettier (printWidth 100).
- Type-check: `tsc --noEmit` in CI.
- Tests: `vitest`/`jest` with coverage ≥85%, deterministic seeds.
- Runtime: avoid `any`; prefer discriminated unions; never throw strings; structured logs.
- Security: no secrets in code; env schema validation (zod/typebox); redact logs; input validation.