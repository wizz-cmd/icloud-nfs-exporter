# [PROJECT NAME]

## Session Rules

- **Start of every session**: Read `HANDOVER.md` before doing anything else. It contains the current project state, what works, what's broken, and the immediate next step.
- **After every commit**: Update `HANDOVER.md` to reflect what changed — especially the "What Works", "What Does NOT Work Yet", "Immediate Next Step", and "Version" sections. Keep it concise and current.

## Project Goal

<!-- One paragraph. What does this project do and for whom? -->

## Core Requirements

<!-- Bullet list. What MUST the project deliver? -->

## Guiding Principles

- **FOSS & digital independence**: All dependencies must be open-source. No lock-in to proprietary tools beyond platform APIs that are strictly necessary.
- **Local-first data**: All data stored on-device by default. No cloud dependency. No network calls without explicit user action. No telemetry without explicit opt-in.
- **Open formats**: Prefer SQLite, JSON, TOML, Markdown, plain text, CSV. Avoid proprietary binary formats. User must be able to extract their data without the app.
- **Robust & fault-tolerant**: Handle errors, network outages, and edge cases gracefully. Never corrupt data.
- **Outstanding UX**: Clear status reporting, sane defaults, helpful error messages. Setup should take minutes, not hours.
- **Reuse before building**: Research what already exists before writing new code. Stand on the shoulders of giants.

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full architecture, design patterns, and references.

**Rules for architecture documentation:**
- All significant design decisions are recorded as ADRs (Architecture Decision Records) in `ARCHITECTURE.md`.
- Every ADR names the design pattern (GoF, POSA, or domain-specific), describes the decision, states the rationale, and links to references.
- ADRs are append-only. If a decision changes, write a new ADR that supersedes the old one.

## Repository Layout

```
project/
├── CLAUDE.md           # This file — AI-assisted development context
├── HANDOVER.md         # Session handover document (always current)
├── ARCHITECTURE.md     # Architecture decisions, patterns, references
├── README.md           # User-facing documentation
├── CHANGELOG.md        # Keep a Changelog format
├── CONTRIBUTING.md     # Development setup, commit format, release process
├── LICENSE             # MIT / Apache 2.0
├── CODE_OF_CONDUCT.md  # Contributor Covenant v2.1
├── SECURITY.md         # Vulnerability reporting process
├── .editorconfig       # Consistent indentation across languages
├── .github/
│   ├── workflows/      # CI + release automation
│   ├── ISSUE_TEMPLATE/ # Bug report + feature request templates
│   └── PULL_REQUEST_TEMPLATE.md
├── src/                # Source code
├── scripts/            # Install / setup / build scripts
├── tests/              # Test suites
├── docs/
│   └── internal/       # Implementation reports, design research (not user-facing)
└── Makefile            # build, test, lint, install, uninstall, docs, help
```

## Preferred Languages

<!-- Adjust per project. The principle: use the right language for each job. -->

- **Prio 1 (scripting/glue)**: Python, bash
- **Prio 2 (platform-native)**: Swift (macOS/iOS), Kotlin (Android), TypeScript (web)
- **Prio 3 (performance-critical)**: Rust, C++, C

## Development Guidelines

### Commits
- Follow [Conventional Commits](https://www.conventionalcommits.org/) format: `type: short description`
- Types: `feat`, `fix`, `docs`, `test`, `refactor`, `chore`
- Push all changes to GitHub after committing

### Testing
- Every component must have tests. No exceptions.
- Tests run in CI on every push and before every release.
- Makefile `test` target runs all test suites across all languages.

### Linting
- Enforce code style with language-native linters (not manual review).
- Makefile `lint` target runs all linters.

### Documentation
- **Source code**: Doc comments on all public API. Language conventions:
  - Swift: `///` with DocC markup (`- Parameter:`, `- Returns:`, `- Throws:`)
  - Rust: `///` and `//!` with rustdoc, `# Examples` on every public function
  - Python: Google-style docstrings with Args/Returns/Raises, type hints on all signatures
- **README.md**: Scannable in 10 seconds. Status badge, one-line description, installation, quick start, CLI reference.
- **Internal docs** go in `docs/internal/`, not `docs/`. Users should not see implementation reports.
- **Never duplicate**. If it's documented once, link to it. If a README and a wiki say the same thing, one will rot.
- **Comments explain WHY, not WHAT**. If the code needs a comment to explain what it does, refactor the code.

### Versioning
- Follow [Semantic Versioning](https://semver.org/).
- Version strings must be consistent across all components. Grep for the old version before releasing.
- Update CHANGELOG.md with every release.

## macOS App Design Rules

<!-- Remove this section if not building a macOS app. -->

### Human Interface Guidelines
- **Menu bar**: Every app must have standard menus (App, Edit, Window, Help) with standard keyboard shortcuts (Cmd+Q, Cmd+W, Cmd+,, Cmd+Z).
- **Settings**: Use SwiftUI `Settings` scene with `TabView` + `.formStyle(.grouped)`. Opened via Cmd+, automatically.
- **Windows**: Freely resizable with sensible minimum size. Remember position across launches.
- **SF Symbols**: Use for all icons. Set `isTemplate = true` on menu bar icons.
- **System fonts**: Use semantic styles (`.font(.headline)`, `.font(.body)`). Never hardcode font sizes.

### Visual
- **Semantic colours only**: Use `.primary`, `.secondary`, `Color.accentColor`, `Color(.windowBackgroundColor)`. Never hardcode RGB.
- **Dark mode**: Test both appearances. Default to system setting.
- **8pt spacing grid**: Standard padding 16pt, section spacing 24pt, compact 8pt.

### Accessibility
- **VoiceOver**: Every interactive element must have `accessibilityLabel`. Group related elements with `accessibilityElement(children: .combine)`.
- **Keyboard**: Every feature must be reachable via keyboard. Add `.keyboardShortcut()` to all buttons.
- **Reduce Motion**: Respect `accessibilityReduceMotion`. Replace animations with crossfades.
- **Contrast**: Minimum 4.5:1 ratio. System semantic colours handle this automatically.
- **Colour is not the only signal**: Always pair colour with icons, labels, or patterns.

### SwiftUI Architecture (macOS 14+)
- Use `@main struct App` with scene declarations, not manual AppKit lifecycle.
- Use `@Observable` for state management (requires macOS 14).
- Use `MenuBarExtra(.window)` for menu bar apps.
- Use `Settings` scene for preferences.
- Drop into AppKit via `NSViewRepresentable` only where SwiftUI has concrete gaps.

### Distribution
- **Code sign** with Developer ID certificate.
- **Notarise** via `xcrun notarytool` — mandatory since macOS Catalina.
- **DMG**: Drag-to-Applications layout. Include README.txt with Gatekeeper workaround for unsigned builds.
- **Auto-updates**: Use [Sparkle](https://sparkle-project.org/) for direct distribution.
- **Homebrew Cask**: Requires code signing and notarisation (mandatory since Homebrew 5.0).

## FOSS Project Standards

Every public repo should include:

| File | Purpose |
|---|---|
| `LICENSE` | MIT or Apache 2.0 |
| `README.md` | CI/release/license/status badges, installation, quick start |
| `CHANGELOG.md` | Keep a Changelog format |
| `CONTRIBUTING.md` | Dev setup, project structure, commit format, release process, Makefile help |
| `CODE_OF_CONDUCT.md` | Contributor Covenant v2.1 |
| `SECURITY.md` | Private vulnerability reporting (GitHub security advisories) |
| `.editorconfig` | Indent style/size per language |
| `.github/ISSUE_TEMPLATE/bug.md` | OS version, repro steps, diagnostics output |
| `.github/ISSUE_TEMPLATE/feature.md` | Problem, proposed solution, alternatives |
| `.github/PULL_REQUEST_TEMPLATE.md` | Summary, related issue, testing checklist |

## Anti-Patterns to Avoid

- **No parrot comments**: `i += 1  // increment i` — delete these.
- **No journal comments**: Changelogs at top of files — that's what git log is for.
- **No commented-out code**: The VCS has the history. Delete it.
- **No over-documentation**: If you can delete a comment and the code is equally clear, delete it.
- **No internal docs in user-facing directories**: Implementation reports go in `docs/internal/`.
- **No hardcoded colours, fonts, or sizes**: Use system semantic values.
- **No telemetry without opt-in**: Zero analytics by default.
- **No network calls without user action**: The app works fully offline.
