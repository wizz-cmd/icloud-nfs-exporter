# macOS App Design Rules -- Reference for Coding Agents

> Synthesized 2026-04-16 by Wit.
> Purpose: Actionable design rules for a coding agent building a native, beautiful, FOSS-aligned macOS application.

---

## 1. Apple Human Interface Guidelines (HIG) -- Key Rules

**Authoritative source:** https://developer.apple.com/design/human-interface-guidelines/designing-for-macos

### 1.1 Core Principles

- **Content is king.** The UI exists to serve the content. Navigation and controls float above; content fills the space.
- **Spaciousness.** macOS has large screens. Use the space. Do not cram elements together. Let content breathe.
- **Flexibility.** Users resize windows, use multiple displays, run many apps simultaneously. Your app must handle all of this gracefully.
- **Familiarity.** Follow platform conventions. Users who cannot find a feature look in the menu bar first. Do not invent novel UI patterns where standard ones exist.

### 1.2 Liquid Glass (macOS 26 Tahoe -- Current Design Language)

Liquid Glass is the most significant visual change since iOS 7, introduced at WWDC 2025. It applies across macOS 26, iOS 26, and all Apple platforms.

**What it is:** A translucent material that refracts and bends light in real time. Not a simple blur -- it uses lensing, specular highlights, and adaptive shadows.

**Golden Rules:**

| Rule | Detail |
|------|--------|
| Navigation layer only | Apply glass to toolbars, tab bars, sidebars, floating action buttons, sheets, popovers. NEVER to content (lists, cards, tables, backgrounds). |
| Never stack glass on glass | Glass cannot properly sample other glass surfaces. Visual artifacts result. |
| Use GlassEffectContainer | When multiple glass elements coexist, wrap them in `GlassEffectContainer` for shared sampling, performance, and morphing support. |
| Never mix variants | Use `.regular` OR `.clear` in a given context -- never both simultaneously. |
| Tint sparingly | Tint ONLY the primary/confirmatory action. When everything is tinted, nothing stands out. |

**What gets Liquid Glass automatically (zero code, just recompile with Xcode 26):**
- Toolbar, Sidebar, Menu bar, Dock
- Window controls, NSPopover, Sheets
- NavigationBar, TabBar (iOS)

**SwiftUI API:**
```swift
// Basic
Button("Action") { }
    .glassEffect()                           // .regular variant, default

// Explicit variant and shape
Text("Label")
    .padding()
    .glassEffect(.regular, in: .capsule)

// Button styles
Button("Cancel") { }.buttonStyle(.glass)           // Secondary actions
Button("Confirm") { }.buttonStyle(.glassProminent)  // Primary actions
    .tint(.blue)

// Container for multiple elements
GlassEffectContainer(spacing: 30) {
    HStack(spacing: 16) {
        Button("Edit") { }.glassEffect()
        Button("Share") { }.glassEffect()
    }
}

// Morphing transitions
@Namespace var ns
GlassEffectContainer {
    Button("Toggle") { withAnimation(.bouncy) { expanded.toggle() } }
        .glassEffect()
        .glassEffectID("toggle", in: ns)
    if expanded {
        Button("Action") { }
            .glassEffect()
            .glassEffectID("action", in: ns)
    }
}
```

**Clear variant prerequisites (ALL three must be true):**
1. Element sits over media-rich content (photos, video)
2. Content will not be negatively affected by dimming layer
3. Content above glass is bold and bright

**Accessibility is automatic.** Reduce Transparency, Increase Contrast, and Reduce Motion are handled by the system for glass effects. No code needed.

### 1.3 Window Management

| Rule | Implementation |
|------|---------------|
| All main windows must be freely resizable | `.windowResizability(.contentMinSize)` (default) |
| Set a minimum size that keeps UI usable | `.frame(minWidth: 600, minHeight: 400)` |
| Set a sensible default size | `.defaultSize(width: 900, height: 600)` |
| Never set a maximum size unless content truly cannot scale | Rare -- almost never do this |
| Support native fullscreen | Green traffic-light button must enter fullscreen or show tile picker |
| Remember window position and size | Use `@SceneStorage` or `NSWindow.setFrameAutosaveName()` |
| Support window restoration on relaunch | Default behavior in SwiftUI -- do not break it |

### 1.4 Sidebar + Detail Layout

The canonical macOS app layout is `NavigationSplitView`:

```swift
NavigationSplitView {
    // Sidebar: list of items, collapsible
    List(selection: $selection) { ... }
} detail: {
    // Detail view for selected item
    DetailView(item: selection)
}
```

**Rules:**
- Sidebar appears on the leading (left) edge
- Sidebar must be collapsible via toolbar button AND keyboard shortcut (Cmd+Ctrl+S)
- Sidebar collapsed state must persist across launches (`@SceneStorage`)
- Sidebar uses translucent material automatically on macOS
- Use `NavigationSplitView`, NEVER the deprecated `NavigationView`

### 1.5 Toolbar

- Toolbar sits at the top of the window, inside the title bar area
- Place search in trailing area using `.searchable()`
- Use SF Symbols for toolbar icons
- Keep toolbar items minimal -- 5-7 items maximum
- Group related items with `ToolbarItemGroup`
- Toolbars automatically get Liquid Glass in macOS 26

### 1.6 Standard Menu Bar

Every app MUST include these menus (in this order):

| Menu | Contents |
|------|----------|
| **App menu** (app name) | About, Settings (Cmd+,), Services, Hide, Quit (Cmd+Q) |
| **File** | New (Cmd+N), Open (Cmd+O), Save (Cmd+S), Close (Cmd+W) -- omit only if not document-based |
| **Edit** | Undo (Cmd+Z), Redo (Cmd+Shift+Z), Cut/Copy/Paste, Select All (Cmd+A), Find (Cmd+F) |
| **View** | Toggle Sidebar, Zoom, Enter Full Screen |
| **Window** | Minimize (Cmd+M), Zoom, Bring All to Front |
| **Help** | Search field + help content |

Add app-specific menus between Edit and Window. Never remove standard menu items.

### 1.7 Settings/Preferences Window

```swift
Settings {
    TabView {
        GeneralSettingsView()
            .tabItem { Label("General", systemImage: "gear") }
        AppearanceSettingsView()
            .tabItem { Label("Appearance", systemImage: "paintbrush") }
        AdvancedSettingsView()
            .tabItem { Label("Advanced", systemImage: "gearshape.2") }
    }
}
```

**Rules:**
- Opened via Cmd+, (this is automatic with SwiftUI `Settings` scene)
- Use `TabView` with icon+label tabs at top
- Each category gets its own tab and source file
- Use `.formStyle(.grouped)` for layout
- Window title is "Settings" (not "Preferences" -- Apple changed this in macOS 13)
- Settings window is NOT resizable (default behavior)

### 1.8 Standard Keyboard Shortcuts

These are non-negotiable. Users expect them:

| Shortcut | Action |
|----------|--------|
| Cmd+Q | Quit |
| Cmd+W | Close window/tab |
| Cmd+, | Settings |
| Cmd+N | New |
| Cmd+O | Open |
| Cmd+S | Save |
| Cmd+Z / Cmd+Shift+Z | Undo / Redo |
| Cmd+C / Cmd+V / Cmd+X | Copy / Paste / Cut |
| Cmd+A | Select All |
| Cmd+F | Find |
| Cmd+M | Minimize |
| Cmd+H | Hide app |
| Cmd+Ctrl+F | Toggle fullscreen |
| Cmd+Ctrl+S | Toggle sidebar |
| Cmd++ / Cmd+- | Zoom in/out |

---

## 2. Technology Choice

### 2.1 SwiftUI -- Recommended Primary Framework

**Current state (2025/2026):** SwiftUI has matured significantly but is not yet a complete AppKit replacement. It is Apple's clear strategic direction.

**What works well:**
- List/OutlineGroup performance: 10,000+ items are snappy
- NavigationSplitView for sidebar+detail layouts
- Rich text editing via `TextEditor` + `AttributedString`
- Liquid Glass integration is automatic
- Settings scene, menu commands, toolbar APIs
- Swift Charts, WebView (native, no NSViewRepresentable needed)
- Dark mode, accessibility, Dynamic Type -- mostly automatic

**Known limitations (as of macOS 26):**
- Cannot subclass NSWindow for custom window behavior
- No direct access to NSEvents or first responder chain
- Font selection panel ("Show Fonts") remains disabled in TextEditor
- Spell-checking in TextEditor is erratic
- Clearing very large lists (50k+) has disproportionate performance cost
- Breaking behavior changes across macOS versions still occur
- Some features still need AppKit bridges (`NSViewRepresentable`)
- `@Observable` macro requires macOS 14+ minimum

**Verdict:** Use SwiftUI as the primary framework. Drop into AppKit via `NSViewRepresentable` only where SwiftUI has concrete gaps. This is the same hybrid approach Apple's own apps use.

### 2.2 AppKit -- For Specific Gaps Only

AppKit has 30+ years of APIs. It does not break between releases. Use it when:
- You need custom `NSWindow` behavior (floating panels, etc.)
- You need advanced text editing beyond TextEditor capabilities
- You need first responder chain manipulation
- You need APIs that SwiftUI simply does not expose

Bridge pattern:
```swift
struct AppKitTextView: NSViewRepresentable {
    func makeNSView(context: Context) -> NSTextView { ... }
    func updateNSView(_ nsView: NSTextView, context: Context) { ... }
}
```

### 2.3 Cross-Platform Alternatives -- Analysis

| Framework | Native Feel | FOSS | Linux | Verdict |
|-----------|------------|------|-------|---------|
| **SwiftUI** | Excellent (it IS native) | Source available (not FOSS) | No | Best for macOS-first |
| **Tauri 2.x** (Rust + WebView) | Good (uses system WebView) | MIT/Apache | Yes | Best FOSS cross-platform option |
| **Qt 6** | Good (can look native) | LGPL/Commercial | Yes | Heavy, complex licensing |
| **GTK4** | Poor on macOS | LGPL | Yes | Not viable for macOS |
| **Electron** | Terrible | MIT | Yes | Explicitly rejected |

**Recommendation for FOSS + native macOS beauty:**

- **Option A (macOS-first):** SwiftUI + AppKit hybrid. Best possible macOS experience. MIT/Apache license the app code. Swift itself is Apache-licensed. The app is FOSS even though the framework is not.
- **Option B (cross-platform priority):** Tauri 2.x. Rust backend + HTML/CSS/JS frontend using system WebView (not bundled Chromium). MIT-licensed. Produces small binaries. Runs on macOS, Linux, Windows. UI will be good but not pixel-perfect macOS native.
- **Option C (compromise):** SwiftUI for macOS, with shared Rust/Swift core logic compiled for Linux separately. UI is platform-specific, business logic is shared.

### 2.4 Minimum Deployment Target

Target macOS 14 (Sonoma) as minimum. This gives access to:
- `@Observable` macro
- Modern NavigationSplitView APIs
- Inspector views
- Improved list performance

For Liquid Glass features, macOS 26 (Tahoe) is required, but they degrade gracefully on older systems.

---

## 3. Typography and Layout

### 3.1 System Fonts

| Font | Use Case |
|------|----------|
| **SF Pro** | All UI text. Variable optical sizing handles Text (<=19pt) and Display (>=20pt) automatically. |
| **SF Mono** | Code, terminal output, monospaced data |
| **SF Pro Rounded** | Friendly UI elements (badges, tags) -- use sparingly |
| **New York** | Serif text for reading-heavy content -- editorial feel |

**Rules:**
- ALWAYS use the system font unless you have a strong brand reason not to
- Never hardcode font sizes in pixels. Use SwiftUI semantic styles:
  ```swift
  .font(.largeTitle)    // 26pt
  .font(.title)         // 22pt
  .font(.title2)        // 17pt
  .font(.title3)        // 15pt
  .font(.headline)      // 13pt semibold
  .font(.body)          // 13pt
  .font(.callout)       // 12pt
  .font(.subheadline)   // 11pt
  .font(.footnote)      // 10pt
  .font(.caption)       // 10pt
  .font(.caption2)      // 10pt light
  ```
- Variable optical sizing is automatic in macOS 11+. The system adjusts letter spacing based on size.
- SF Pro Text (<=19pt) has wider letter spacing for legibility
- SF Pro Display (>=20pt) has tighter letter spacing for headlines

### 3.2 SF Symbols

- Library of 6,900+ symbols, designed to align with SF Pro text
- Available in 9 weights and 3 scales (small, medium, large)
- Use `Image(systemName: "symbol.name")` in SwiftUI
- Symbols automatically match adjacent text weight and size
- Use SF Symbols app (free from Apple) to browse available symbols
- Prefer SF Symbols over custom icons whenever possible
- Create custom symbols only when no SF Symbol exists, using the SF Symbols template

**Rendering modes:**
- `.monochrome` -- single color (default)
- `.hierarchical` -- depth through opacity layers
- `.palette` -- custom multi-color
- `.multicolor` -- Apple-defined colors

```swift
Image(systemName: "chart.bar.fill")
    .symbolRenderingMode(.hierarchical)
    .foregroundStyle(.blue)
```

### 3.3 Spacing Grid

- Use an **8pt base grid** for spacing and padding
- Standard padding: 16pt (2 grid units)
- Compact padding: 8pt (1 grid unit)
- Section spacing: 24pt (3 grid units)
- SwiftUI `.padding()` defaults are generally correct for the platform
- Use `.padding(.horizontal, 16)` for explicit control
- Content insets from window edge: 20pt

---

## 4. Colors and Dark Mode

### 4.1 Semantic Colors -- Always Use These

```swift
// Text
.foregroundStyle(.primary)        // Main text
.foregroundStyle(.secondary)      // Supplementary text
.foregroundStyle(.tertiary)       // Disabled/placeholder text

// Backgrounds
Color(.windowBackgroundColor)     // Window background
Color(.controlBackgroundColor)    // Input field backgrounds
Color(.underPageBackgroundColor)  // Behind content

// Accent
Color.accentColor                 // User's chosen accent color
```

**Rules:**
- NEVER use hardcoded colors (e.g., `Color.black`, `Color.white`) for UI chrome
- Always use semantic/system colors that adapt to Light/Dark mode automatically
- For brand colors, define both Light and Dark variants in the Asset Catalog
- System colors like `.systemRed`, `.systemBlue` automatically adjust for dark mode
- Test BOTH appearances. Always. No exceptions.

### 4.2 Dark Mode Implementation

```swift
// Respect system setting (default -- do not override)
// The app automatically follows System Preferences

// To read current mode:
@Environment(\.colorScheme) var colorScheme

// To force a specific mode (rare, usually wrong):
.preferredColorScheme(.dark)
```

**Rules:**
- Default to system appearance. Do not force dark or light.
- If offering an in-app toggle, provide three options: Light, Dark, System (default)
- Never assume dark backgrounds are black. Use semantic background colors.
- Ensure sufficient contrast (WCAG AA minimum: 4.5:1 for normal text)

---

## 5. Accessibility

### 5.1 VoiceOver

- SwiftUI provides good VoiceOver support by default for standard controls
- Every interactive element must be reachable and activatable via VoiceOver
- Provide meaningful labels:
  ```swift
  Button(action: save) {
      Image(systemName: "square.and.arrow.down")
  }
  .accessibilityLabel("Save document")
  ```
- Group related elements:
  ```swift
  HStack { ... }
      .accessibilityElement(children: .combine)
  ```
- Use `accessibilityValue` for state information
- Use `accessibilityHint` for non-obvious actions
- Navigation order must be logical (generally top-to-bottom, left-to-right)
- In macOS 26: use `.accessibilityDefaultFocus()` to suggest initial focus for new scenes

### 5.2 Keyboard Navigation

- **Every feature must be accessible via keyboard.** No exceptions.
- Tab moves focus between controls
- Arrow keys navigate within lists, tables, outlines
- Space activates buttons, checkboxes
- Return/Enter confirms default action
- Escape dismisses sheets/popovers/alerts
- Standard shortcuts (Section 1.8) must all work
- Custom keyboard shortcuts must not conflict with system shortcuts
- VoiceOver cursor must follow standard keyboard navigation

### 5.3 Other Accessibility Features

| Feature | Requirement |
|---------|-------------|
| **Reduce Motion** | Disable non-essential animations. Liquid Glass handles this automatically. |
| **Increase Contrast** | System colors handle this. Test with setting enabled. |
| **Reduce Transparency** | Glass effects automatically become frostier. |
| **Dynamic Type** | Support font scaling. Use semantic font styles. |
| **Color blindness** | Never convey information by color alone. Add icons, labels, or patterns. |

---

## 6. Data Sovereignty and FOSS Alignment

### 6.1 Local-First Data Storage

| Rule | Implementation |
|------|---------------|
| Data lives on-device by default | Store in `~/Library/Application Support/AppName/` or `~/Documents/` |
| SQLite for structured data | Use via SwiftData, GRDB, or raw SQLite. Self-contained, zero-config, battle-tested. |
| Open file formats | Prefer: SQLite, JSON, Markdown, plain text, CSV. Avoid proprietary binary formats. |
| No cloud dependency | App must work fully offline. Cloud sync is optional, additive. |
| User owns their data | Provide export (JSON, CSV, Markdown) and import capabilities |
| No vendor lock-in | Data must be extractable without the app. Document the schema. |

### 6.2 Privacy and Telemetry

- **No telemetry by default.** Zero analytics, zero crash reporting unless user opts in.
- If offering telemetry: explicit opt-in, not opt-out. Show exactly what is collected.
- No network calls that the user does not initiate or explicitly approve
- Respect macOS privacy permissions (Contacts, Calendar, etc.) -- request only what you need
- Store secrets in Keychain, never in plaintext files

### 6.3 Open Source Licensing

| License | Best For | Key Property |
|---------|----------|--------------|
| **MIT** | Maximum adoption, permissive | Anyone can do anything, just keep copyright notice |
| **Apache 2.0** | Enterprise-friendly, patent protection | Like MIT + explicit patent grant |
| **GPL v3** | Ensure derivatives stay open | Copyleft -- derivatives must also be GPL |
| **AGPL v3** | Network services | GPL + network use triggers copyleft |
| **MPL 2.0** | File-level copyleft | Modified files must be open, new files can be proprietary |

**Recommendation:** **MIT** or **Apache 2.0** for a macOS app. Maximizes adoption and community contribution. Swift itself is Apache 2.0. Most successful FOSS macOS apps use MIT.

**Note:** GPL is fine philosophically but complicates App Store distribution (Apple's DRM terms create tension with GPL). For direct distribution only, GPL works.

---

## 7. Distribution

### 7.1 Mac App Store vs Direct Distribution

| Aspect | App Store | Direct Distribution |
|--------|-----------|-------------------|
| Discoverability | Good for consumers | Requires marketing |
| Revenue cut | 30% (15% small business) | 0% |
| Sandboxing | Required | Optional (but recommended) |
| Update mechanism | Automatic | Must build your own (Sparkle framework) |
| GPL compatibility | Problematic (DRM) | No issues |
| FOSS alignment | Poor (Apple controls distribution) | Good |

**Recommendation:** Direct distribution for FOSS alignment. Offer both if resources permit.

### 7.2 Direct Distribution Requirements

1. **Apple Developer Account** ($99/year) -- required for code signing and notarization
2. **Code signing** with Developer ID certificate
3. **Notarization** -- mandatory since macOS Catalina. Without it, Gatekeeper blocks the app.
4. **Stapling** -- attach notarization ticket to the binary

**Process:**
```bash
# 1. Archive in Xcode: Product > Archive > Distribute App > Direct Distribution

# 2. Store credentials
xcrun notarytool store-credentials "AC_PASSWORD" \
    --apple-id $EMAIL --team-id $TEAM_ID

# 3. Create DMG
brew install create-dmg
create-dmg \
    --volname "MyApp" \
    --window-pos 200 120 \
    --window-size 600 400 \
    --icon-size 100 \
    --app-drop-link 425 178 \
    "MyApp.dmg" "build/"

# 4. Notarize
xcrun notarytool submit MyApp.dmg \
    --keychain-profile "AC_PASSWORD" --wait

# 5. Staple
xcrun stapler staple MyApp.dmg
```

### 7.3 Homebrew Cask

**Critical change (2025-2026):** Homebrew now REQUIRES code signing and notarization for Casks. Unsigned casks will be removed from the official tap by September 2026. The `--no-quarantine` flag is being removed.

**Submission:**
```ruby
# Formula: Casks/my-app.rb
cask "my-app" do
  version "1.0.0"
  sha256 "abc123..."
  url "https://github.com/user/repo/releases/download/v#{version}/MyApp-#{version}.dmg"
  name "My App"
  desc "Description of the app"
  homepage "https://myapp.example.com"
  app "MyApp.app"
end
```

Submit PR to `homebrew/homebrew-cask` repository.

### 7.4 Auto-Updates

For direct distribution, use **Sparkle** (https://sparkle-project.org/):
- De facto standard for macOS app auto-updates outside the App Store
- MIT licensed
- EdDSA signature verification
- Supports delta updates
- SwiftUI integration available

---

## 8. Exemplary FOSS macOS Apps

These apps demonstrate what "open source AND beautiful native macOS UI" looks like. Study them.

### Tier 1: Gold Standard

| App | What It Is | Tech | License | Stars | Why Exemplary |
|-----|-----------|------|---------|-------|---------------|
| **[CodeEdit](https://github.com/CodeEditApp/CodeEdit)** | Code editor | SwiftUI + AppKit | MIT | 22k+ | "Looks like Apple built it." Meticulous HIG adherence. |
| **[IINA](https://iina.io/)** | Media player | Swift + AppKit | GPL v3 | 38k+ | The definitive example of a beautiful FOSS macOS app. |
| **[Maccy](https://github.com/p0deje/Maccy)** | Clipboard manager | Swift + AppKit | MIT | 14k+ | Lightweight, privacy-first, native menu bar integration. |

### Tier 2: Excellent

| App | What It Is | Tech | License | Why Notable |
|-----|-----------|------|---------|-------------|
| **[Rectangle](https://github.com/rxhanson/Rectangle)** | Window manager | Swift + AppKit | MIT | Clean preferences, keyboard-first design |
| **[AltTab](https://github.com/lwouis/alt-tab-macos)** | Window switcher | Swift + AppKit | GPL v3 | Feels native despite adding non-native behavior |
| **[Ice](https://github.com/jordanbaird/Ice)** | Menu bar manager | SwiftUI | MIT | Modern SwiftUI, clean design |
| **[FSNotes](https://github.com/glushchenko/fsnotes)** | Notes manager | Swift + AppKit | MIT | Markdown-native, local-first |
| **[Pika](https://github.com/nicklama/pika)** | Color picker | SwiftUI | MIT | Small, beautiful, SwiftUI showcase |
| **[Itsycal](https://github.com/sfsam/Itsycal)** | Menu bar calendar | Obj-C + AppKit | MIT | Perfect menu bar integration |

### Tier 3: Study for Specific Patterns

| App | Study For |
|-----|-----------|
| **[Whisky](https://github.com/Whisky-App/Whisky)** | SwiftUI app structure, modern macOS UI patterns |
| **[MonitorControl](https://github.com/MonitorControl/MonitorControl)** | Menu bar utility, system integration |
| **[Swiftcord](https://github.com/nicklama/swiftcord)** | Complex SwiftUI chat interface |
| **[Ollama (macOS)](https://github.com/ollama/ollama)** | Menu bar utility for AI |

---

## 9. Design Checklist for Agent Implementation

Before shipping any screen, verify:

### Layout
- [ ] Window is freely resizable with sensible minimum size
- [ ] Sidebar is collapsible and state persists
- [ ] Content fills available space without awkward gaps
- [ ] 8pt grid alignment for spacing
- [ ] Toolbar has <= 7 items

### Visual
- [ ] Uses semantic/system colors exclusively (no hardcoded colors)
- [ ] Dark mode tested and correct
- [ ] Liquid Glass on navigation layer only (if targeting macOS 26)
- [ ] SF Symbols used for all icons (no custom icons unless necessary)
- [ ] System font with semantic text styles

### Interaction
- [ ] All standard keyboard shortcuts work (Cmd+Q, Cmd+W, Cmd+,, etc.)
- [ ] Every feature reachable via keyboard
- [ ] Drag and drop works where contextually appropriate
- [ ] Menu bar contains all standard menus
- [ ] Settings window uses TabView with Cmd+, shortcut

### Accessibility
- [ ] VoiceOver navigable -- every element labeled and reachable
- [ ] Color is never the only way to convey information
- [ ] Reduce Motion respected
- [ ] Minimum 4.5:1 contrast ratio for text

### Data
- [ ] All data stored locally by default
- [ ] Open file format (SQLite, JSON, Markdown)
- [ ] Export capability exists
- [ ] Import capability exists
- [ ] No network calls without user action
- [ ] No telemetry without explicit opt-in

### Distribution
- [ ] Code signed with Developer ID
- [ ] Notarized via `notarytool`
- [ ] DMG created with drag-to-Applications layout
- [ ] Sparkle integrated for auto-updates
- [ ] Homebrew Cask formula prepared

---

## 10. SwiftUI App Template Skeleton

```swift
import SwiftUI

@main
struct MyApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
        .defaultSize(width: 900, height: 600)
        .commands {
            SidebarCommands()
            // Custom commands here
        }

        Settings {
            SettingsView()
        }
    }
}

struct ContentView: View {
    @State private var selection: Item.ID?

    var body: some View {
        NavigationSplitView {
            SidebarView(selection: $selection)
                .frame(minWidth: 200)
        } detail: {
            if let selection {
                DetailView(itemID: selection)
            } else {
                Text("Select an item")
                    .foregroundStyle(.secondary)
            }
        }
        .frame(minWidth: 600, minHeight: 400)
    }
}

struct SettingsView: View {
    var body: some View {
        TabView {
            GeneralSettings()
                .tabItem { Label("General", systemImage: "gear") }
            AppearanceSettings()
                .tabItem { Label("Appearance", systemImage: "paintbrush") }
            DataSettings()
                .tabItem { Label("Data", systemImage: "externaldrive") }
        }
        .frame(width: 450)
    }
}
```

---

## Sources

- [Apple HIG: Designing for macOS](https://developer.apple.com/design/human-interface-guidelines/designing-for-macos)
- [Apple HIG: What's New](https://developer.apple.com/design/whats-new/)
- [Apple: Liquid Glass Announcement](https://www.apple.com/newsroom/2025/06/apple-introduces-a-delightful-and-elegant-new-software-design/)
- [Liquid Glass in Swift: Official Best Practices](https://dev.to/diskcleankit/liquid-glass-in-swift-official-best-practices-for-ios-26-macos-tahoe-1coo)
- [Liquid Glass Reference (GitHub)](https://github.com/conorluddy/LiquidGlassReference)
- [SwiftUI for Mac 2025](https://troz.net/post/2025/swiftui-mac-2025/)
- [AppKit vs SwiftUI: Stable vs Shiny](https://milen.me/writings/appkit-vs-swiftui-stable-vs-shiny/)
- [Explainer: AppKit and SwiftUI (2026)](https://eclecticlight.co/2026/04/04/explainer-appkit-and-swiftui/)
- [SwiftUI 2025: What's Fixed, What's Not](https://juniperphoton.substack.com/p/swiftui-2025-whats-fixed-whats-not)
- [Tauri 2.0](https://tauri.app/)
- [Cross-Platform Dev Tools Comparison 2026](https://codenote.net/en/posts/cross-platform-dev-tools-comparison-2026/)
- [Apple: SF Symbols](https://developer.apple.com/sf-symbols/)
- [Apple: Fonts](https://developer.apple.com/fonts/)
- [Apple: VoiceOver Evaluation Criteria](https://developer.apple.com/help/app-store-connect/manage-app-accessibility/voiceover-evaluation-criteria/)
- [WWDC25: Make Your Mac App More Accessible](https://developer.apple.com/videos/play/wwdc2025/229/)
- [Apple: Notarizing macOS Software](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
- [Homebrew 5.0.0 Changes](https://workbrew.com/blog/homebrew-5-0-0)
- [Publishing Mac Apps Outside the App Store](https://dev.to/kopiro/how-to-correctly-publish-your-mac-apps-outside-of-the-app-store-38a)
- [Open Source License Comparison 2025](https://yahyou.co/open-source-license-comparison-mit-gpl-apache/)
- [Open Source macOS Apps (GitHub)](https://github.com/serhii-londar/open-source-mac-os-apps)
- [CodeEdit (GitHub)](https://github.com/CodeEditApp/CodeEdit)
- [IINA](https://iina.io/)
- [Maccy (GitHub)](https://github.com/p0deje/Maccy)

