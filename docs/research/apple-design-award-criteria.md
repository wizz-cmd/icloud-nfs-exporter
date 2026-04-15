# What Makes a macOS App Worthy of an Apple Design Award

Research compiled 2026-04-16 to inform the design direction of iCloud NFS Exporter.

---

## 1. ADA Categories and What Apple Looks For

Apple's Design Awards recognise one app and one game across six categories:

| Category | What Apple evaluates |
|---|---|
| **Delight and Fun** | Memorable, engaging, satisfying experiences enhanced by Apple technologies |
| **Innovation** | State-of-the-art experiences through novel use of Apple APIs |
| **Interaction** | Intuitive interfaces and effortless controls tailored to the platform |
| **Inclusivity** | Great experience for all backgrounds, abilities, and languages |
| **Social Impact** | Meaningfully improving people's lives |
| **Visuals and Graphics** | Stunning imagery, skilfully drawn interfaces, cohesive theme |

**Common traits across all winners:** they transform how users interact with a domain rather than wrapping existing functionality. They blend emotional engagement with utility. They use Apple technologies (SwiftUI, SF Symbols, Metal, App Clips, Dynamic Island) in purposeful rather than gratuitous ways.

### Recent Relevant Winners and Finalists

- **iA Writer** (2025, Interaction finalist) — distraction-free writing, deep iCloud sync, custom keyboard. A true macOS-native utility.
- **Play** (2025, Innovation winner) — SwiftUI prototyping tool that exports directly to Xcode.
- **Crouton** (2024, Interaction winner) — recipe manager on iPhone, iPad, Mac, Watch, and Vision Pro. Clean information hierarchy.
- **Copilot Money** (2024, Innovation finalist) — personal finance on iPhone and Mac.
- **Speechify** (2025, Inclusivity winner) — 50+ languages, Dynamic Type, VoiceOver, reduced cognitive load.

**The pattern:** utility apps that reach finalist status have impeccable multi-platform consistency while feeling native on each platform, use Apple APIs purposefully, and treat accessibility as first-class.

---

## 2. macOS Human Interface Guidelines Essentials

### Menu Bar

Every Mac app must have a menu bar. It is the primary command-discovery mechanism.

- **Required menus:** App (About, Settings Cmd+,, Services, Hide, Quit), Edit, Window, Help.
- **App menu:** must include About, Settings/Preferences (Cmd+,), Services, Hide/Show, and Quit.
- App-specific menus go between Edit and View, or between View and Window.

### Settings Window

- Use `Form` with `.formStyle(.grouped)` organised by `Section`.
- Keep the window opaque (not translucent).
- Disable minimise/maximise buttons without removing them.
- Use the `gearshape` SF Symbol for General, `gearshape.2` for Advanced.
- Do not close on Escape — treat as a regular window.
- Include in the Window menu.
- Multi-pane settings: use `NavigationSplitView` with a sidebar.

### Dark Mode

- Use semantic system colours (`.primary`, `.secondary`, `.accentColor`), never hardcoded RGB.
- Set `isTemplate = true` on menu bar icons for automatic light/dark adaptation.
- Test both appearances systematically.

### SF Symbols

- Use the system symbol library for all icons.
- Symbols scale automatically with Dynamic Type and match San Francisco font weight.
- Prefer multicolour or hierarchical rendering modes for visual depth.
- Never ship raster icons for UI elements that have SF Symbol equivalents.

### Liquid Glass (macOS 26 Tahoe)

Apple's most significant visual redesign since iOS 7. Translucent material that reflects and refracts surroundings, dynamically adapts between light and dark. Extends to buttons, switches, sliders, tab bars, and sidebars. Apps targeting macOS Tahoe should adopt this design language.

---

## 3. What Distinguishes a Truly Native macOS App

Specific signals that separate native from cross-platform:

### Typography
- Uses San Francisco (SF Pro) via dynamic system font variants.
- Never ships custom fonts for UI text.
- Respects variable font optical sizing.

### System Colours
- Uses semantic colours that adapt to appearance, contrast, and accessibility.
- Never uses hardcoded hex values for UI chrome.

### Keyboard Shortcuts
- Standard shortcuts work: Cmd+Q, Cmd+W, Cmd+, (Settings), Cmd+H, Cmd+Z/Shift+Z.
- App-specific shortcuts are discoverable in menus with standard modifier glyphs.
- Every interactive element reachable via keyboard.

### Drag and Drop
- Accepts files on relevant surfaces.
- Standard modifier behaviours: Option to copy, Option+Cmd for alias.
- Drop zones paired with clickable alternatives for accessibility.

### Window Management
- Full user control over size, position, and lifecycle.
- Window state restoration — windows reopen where users left them.
- Support "Stay on Top" via Window menu where appropriate.

### Animations
- Uses system-provided transitions.
- Respects Reduce Motion preference (`accessibilityReduceMotion`).
- No janky custom animations that feel foreign to the platform.

### System Integration
- Continuity: Handoff, Universal Clipboard.
- Spotlight: registers searchable content via Core Spotlight.
- Notifications: uses `UNUserNotificationCenter` with actionable notifications.
- Number and date formatting: properly localised.

---

## 4. Accessibility (Increasingly a Gating Factor)

### Accessibility Nutrition Labels (2025)
App Store product pages now display which accessibility features an app supports — VoiceOver, Voice Control, Larger Text, Sufficient Contrast, Reduced Motion, Captions. This creates public accountability.

### VoiceOver on macOS
- macOS uses container-based navigation (faster than element-by-element).
- Use `accessibilityElement(children: .contain)` for logical grouping.
- Use `accessibilityElement(children: .combine)` to merge title+button pairs.
- Use `accessibilitySortPriority` to control reading order.
- Implement Accessibility Rotors for quick jumping between content types.
- All hover-only interactions are inaccessible — provide alternative access.

### Keyboard Navigation
- Every interactive element must be reachable and operable via keyboard.
- Add `.keyboardShortcut()` modifiers.
- Number fields should support arrow keys (±1) and Option+arrow (±10).

### Reduced Motion
- Replace animated transitions with crossfades when enabled.
- Check `accessibilityReduceMotion` environment value.

### Contrast
- Use sufficient contrast ratios.
- System semantic colours handle this automatically; custom colours need manual verification.

---

## 5. Menu Bar App UX Patterns

### NSMenu vs NSPopover

The strong recommendation from experienced developers: use `NSMenu` with embedded `NSHostingView` rather than `NSPopover`. NSPopover has a slight delay, does not dismiss naturally, and looks like a floating app rather than a system utility. NSMenu provides instant open, standard dismiss behaviour, and native animations.

### What the Best Menu Bar Apps Do

Patterns from iStat Menus, Bartender, 1Password, and other exemplars:

1. **At-a-glance status** in the menu bar itself — icon badges, mini-graphs, or small text.
2. **Focused panel** on click — not an entire app window. Shows more detail without overwhelming.
3. **Global keyboard shortcut** to toggle the panel.
4. **"Show in Menu Bar" preference** so users control visibility.
5. **Settings in a proper window** (Cmd+,), not crammed into the popover.
6. **Right-click context menu** on the status item with Quit, Preferences, quick actions.
7. **Dismissible** by clicking outside, pressing Escape, or clicking the icon again.

### Status Bar Icon

- Set `isTemplate = true` so the icon adapts to light/dark mode and menu bar tinting.
- Use a simple, recognisable silhouette — no colour, no detail at 18×18pt.
- Consider dynamic icon states (e.g., fill level, activity indicator).

### Architecture

- `LSUIElement = true` — no Dock icon for pure menu bar utilities.
- Build the Settings window early — forces clean separation and configurability.
- Expect roughly 70% SwiftUI, 30% AppKit for menu bar apps on macOS.

---

## 6. SwiftUI Best Practices for macOS

- **Settings:** use SwiftUI's `Settings` scene or Sindresorhus's `Settings` package.
- **NavigationSplitView:** integrates with macOS translucent sidebar automatically.
- **Inspector modifier:** `.inspector(isPresented:content:)` for property panels.
- **Window restoration:** support `Customizing window styles and state-restoration behavior` APIs.
- **@Observable / @Bindable:** target macOS 14+ to leverage modern Swift features.
- **Technology mix:** SwiftUI for views and state; AppKit for system integration (NSStatusBar, NSMenu, NSPopover).

---

## Sources

- [Apple Design Awards 2025](https://developer.apple.com/design/awards/)
- [Apple Design Awards 2024](https://developer.apple.com/design/awards/2024/)
- [HIG: The Menu Bar](https://developer.apple.com/design/human-interface-guidelines/the-menu-bar)
- [HIG: Designing for macOS](https://developer.apple.com/design/human-interface-guidelines/designing-for-macos)
- [HIG: Accessibility](https://developer.apple.com/design/human-interface-guidelines/accessibility)
- [WWDC 2025: Make your Mac app more accessible](https://developer.apple.com/videos/play/wwdc2025/229/)
- [Sindresorhus HIG Extras](https://github.com/sindresorhus/human-interface-guidelines-extras)
- [Sindresorhus Settings Package](https://github.com/sindresorhus/Settings)
- [Apple Liquid Glass Design Language](https://www.apple.com/newsroom/2025/06/apple-introduces-a-delightful-and-elegant-new-software-design/)
- [Accessibility Nutrition Labels](https://developer.apple.com/help/app-store-connect/manage-app-accessibility/overview-of-accessibility-nutrition-labels/)
