# ST2.2 migration: Rimelight Space tokens to `rimelight_design`

Read-only analysis for step ST2.2 of `rimelight-space/docs/plan.md`. No code, git or cargo was run, and nothing in Space or the design package was edited.

Sources read:
- Space: `crates/rimelight_ui/ui/generated/tokens.slint` (generated, 214 lines), `ui/typography.slint`, `src/slint_ui/codegen.rs`, `src/tokens/{primitive,semantic,component,typography,contrast}.rs`, `src/slint_ui/theme.rs`, `src/theme.rs`, `src/motion.rs`, `src/icon.rs`, `src/slint_ui/tile_drop.rs`, `src/slint_ui/app_shell.rs`, `ui/app-shell.slint`, `ui/components/sidebar.slint`, `apps/gallery`.
- Design: `packages/design/crate/ui/tokens.slint`, `crate/src/generated.rs`, `crate/src/lib.rs`, `tokens.json` (contrast section), `generate.ts` (check mode and contrast logic), `SCHEMA.md`.

Conventions: "Space" means `crates/rimelight_ui` in the Space repo. "design" means `rimelight_design`. Line numbers refer to the file named in the same sentence. In colour rows the Slint form is `dark ? <dark value> : <light value>`. Length rows marked "x zoom-factor" are multiplied by Space's interface zoom at run time.

## Headline numbers

- Space `Tokens` global: 180 properties (85 colour, 66 length, 4 duration, 11 int, 2 float, 9 string, 2 bool, 1 `ThemeChoice`). Of these, 162 are equal to a design property (same name or mapped name, same value, including `theme` and `dark`), 11 differ (2 owner-approved changes: radius and phone breakpoint; 9 font-family strings), and 7 have no design equivalent (3 panel-light colours and 4 runtime zoom and text-scale properties).
- Space `Type` global: 9 text styles plus `scaled()`. Sizes, line heights and weights match the design; all 9 family strings differ (section 1b).
- Slint uses: about 2,600 `Tokens.*` references and about 1,050 `Type.*` references across the 109 `.slint` files that import `Tokens`. The `Type.*` count includes 2 false positives from `InputType.password` and `InputType.text` in `ui/components/input.slint:202`, so about 1,046 are real.
- Only 31 Slint literal lengths are multiplied by `zoom-factor` outside the token global (sidebar 27, slider 3, status bar 1). About 1,450 other non-comment Slint lines carry literal `px` values that never scale.

## 1. Every property in Space's `Tokens` and `Type` globals

### 1a. `Tokens` (180 rows, Space file order)

Row source: Space `ui/generated/tokens.slint` lines 10-176. Design lookup: `crate/ui/tokens.slint`. "Equal" compares the value text after removing ` * zoom-factor`, so a length row marked "yes" is the same number of logical pixels as the design. Mapped names come from the rename map in section 4.

Summary of "no" rows:
- `theme`: same value. The design writes `ThemeChoice.system` and Space writes `system`. Treated as equal.
- `dark`: identical expression. Treated as equal.
- `radius-control`: 5 px (Space, `tokens.slint:120`) vs 6 px (design). Owner-approved change.
- `size-phone-breakpoint`: 600 px (Space, `tokens.slint:162`) vs 640 px (design). Owner-approved change.
- 9 `type-*-family` rows: `"Noto Sans"` (Space) vs `"Noto Sans, sans-serif"` (design). Not owner-approved. See gap 2.
- Missing in design (7): `text-scale-percent`, `text-factor`, `zoom-percent`, `zoom-factor` (runtime state, section 3c), and `panel-light-surface`, `panel-light-text`, `panel-light-muted` (section 3a).

| Space name | Space type | Space value | design name | design value | equal |
|---|---|---|---|---|---|
| `theme` | ThemeChoice | `system` | theme | `ThemeChoice.system` | yes (same enum value, design writes the enum prefix) |
| `dark` | bool | `theme == ThemeChoice.dark || (theme == ThemeChoice.system && Palette.color-scheme == ColorScheme.dark)` | dark | identical expression | yes |
| `reduced-motion` | bool | `false` | reduced-motion | `false` | yes |
| `text-scale-percent` | int | `100` | missing | `-` | missing |
| `text-factor` | float | `text-scale-percent * 1.0 / 100.0` | missing | `-` | missing |
| `zoom-percent` | int | `100` | missing | `-` | missing |
| `zoom-factor` | float | `zoom-percent * 1.0 / 100.0` | missing | `-` | missing |
| `surface-base` | color | `dark ? #121316 : #F2F1EE` | surface-base | `dark ? #121316 : #F2F1EE` | yes |
| `surface-raised` | color | `dark ? #1B1E24 : #FFFFFF` | surface-raised | `dark ? #1B1E24 : #FFFFFF` | yes |
| `surface-sunken` | color | `dark ? #0A0C10 : #F3F9FF` | surface-sunken | `dark ? #0A0C10 : #F3F9FF` | yes |
| `surface-overlay` | color | `dark ? #1B1E24 : #FFFFFF` | surface-overlay | `dark ? #1B1E24 : #FFFFFF` | yes |
| `text-primary` | color | `dark ? #F2F1EE : #101014` | text-primary | `dark ? #F2F1EE : #101014` | yes |
| `text-muted` | color | `dark ? #9AA4B2 : #5E6B7C` | text-muted | `dark ? #9AA4B2 : #5E6B7C` | yes |
| `text-faint` | color | `dark ? #6B7587 : #969A9E` | text-faint | `dark ? #6B7587 : #969A9E` | yes |
| `text-on-accent` | color | `dark ? #0B0B0C : #FFFFFF` | text-on-accent | `dark ? #0B0B0C : #FFFFFF` | yes |
| `border-subtle` | color | `dark ? #2F3540 : #BDBFC4` | border-subtle | `dark ? #2F3540 : #BDBFC4` | yes |
| `border-track` | color | `dark ? #5E6B7C : #5E6B7C` | border-track | `dark ? #5E6B7C : #5E6B7C` | yes |
| `border-strong` | color | `dark ? #7F8A9B : #7A8594` | border-strong | `dark ? #7F8A9B : #7A8594` | yes |
| `accent-default` | color | `dark ? #4DA0FF : #0D7FFF` | accent-default | `dark ? #4DA0FF : #0D7FFF` | yes |
| `accent-hover` | color | `dark ? #8DBBFF : #0968DA` | accent-hover | `dark ? #8DBBFF : #0968DA` | yes |
| `accent-active` | color | `dark ? #5084DB : #1659C4` | accent-active | `dark ? #5084DB : #1659C4` | yes |
| `accent-text` | color | `dark ? #4DA0FF : #0968DA` | accent-text | `dark ? #4DA0FF : #0968DA` | yes |
| `accent-fill` | color | `dark ? #4DA0FF : #1659C4` | accent-fill | `dark ? #4DA0FF : #1659C4` | yes |
| `accent-fill-hover` | color | `dark ? #8DBBFF : #164FA8` | accent-fill-hover | `dark ? #8DBBFF : #164FA8` | yes |
| `accent-fill-active` | color | `dark ? #5084DB : #123F80` | accent-fill-active | `dark ? #5084DB : #123F80` | yes |
| `focus-ring` | color | `dark ? #4DA0FF : #0D7FFF` | focus-ring | `dark ? #4DA0FF : #0D7FFF` | yes |
| `status-success` | color | `dark ? #3FD6B8 : #0A7563` | status-success | `dark ? #3FD6B8 : #0A7563` | yes |
| `status-warning` | color | `dark ? #FFC14D : #8A5800` | status-warning | `dark ? #FFC14D : #8A5800` | yes |
| `status-danger` | color | `dark ? #FF6259 : #B3261E` | status-danger | `dark ? #FF6259 : #B3261E` | yes |
| `status-danger-active` | color | `dark ? #E8483F : #8A1D17` | status-danger-active | `dark ? #E8483F : #8A1D17` | yes |
| `status-info` | color | `dark ? #9DB8FF : #3B5BA9` | status-info | `dark ? #9DB8FF : #3B5BA9` | yes |
| `rail-bg` | color | `dark ? #14386C : #14386C` | extension-brand-rail-bg | `dark ? #14386C : #14386C` | yes |
| `rail-icon-inactive` | color | `dark ? #A3B8D6 : #A3B8D6` | extension-brand-rail-icon-inactive | `dark ? #A3B8D6 : #A3B8D6` | yes |
| `rail-icon-active` | color | `dark ? #FFFFFF : #FFFFFF` | extension-brand-rail-icon-active | `dark ? #FFFFFF : #FFFFFF` | yes |
| `sidebar-bg` | color | `dark ? #0F4E94 : #0F4E94` | extension-brand-sidebar-bg | `dark ? #0F4E94 : #0F4E94` | yes |
| `sidebar-text` | color | `dark ? #E1E8F2 : #E1E8F2` | extension-brand-sidebar-text | `dark ? #E1E8F2 : #E1E8F2` | yes |
| `sidebar-text-muted` | color | `dark ? #D0DCEF : #D0DCEF` | extension-brand-sidebar-text-muted | `dark ? #D0DCEF : #D0DCEF` | yes |
| `sidebar-icon` | color | `dark ? #E6ECF5 : #E6ECF5` | extension-brand-sidebar-icon | `dark ? #E6ECF5 : #E6ECF5` | yes |
| `sidebar-hover-bg` | color | `dark ? #1158A7 : #1158A7` | extension-brand-sidebar-hover-bg | `dark ? #1158A7 : #1158A7` | yes |
| `sidebar-selected-bg` | color | `dark ? #1364BE : #1364BE` | extension-brand-sidebar-selected-bg | `dark ? #1364BE : #1364BE` | yes |
| `sidebar-selected-text` | color | `dark ? #FFFFFF : #FFFFFF` | extension-brand-sidebar-selected-text | `dark ? #FFFFFF : #FFFFFF` | yes |
| `sidebar-selected-indicator` | color | `dark ? #FFFFFF : #FFFFFF` | extension-brand-sidebar-selected-indicator | `dark ? #FFFFFF : #FFFFFF` | yes |
| `sidebar-link` | color | `dark ? #BDDBFF : #BDDBFF` | extension-brand-sidebar-link | `dark ? #BDDBFF : #BDDBFF` | yes |
| `sidebar-divider` | color | `dark ? #0A3F78 : #0A3F78` | extension-brand-sidebar-divider | `dark ? #0A3F78 : #0A3F78` | yes |
| `selection` | color | `dark ? #0D7FFF99 : #0D7FFF99` | selection | `dark ? #0D7FFF99 : #0D7FFF99` | yes |
| `scrim` | color | `dark ? #000000AA : #00000069` | scrim | `dark ? #000000AA : #00000069` | yes |
| `disabled-surface` | color | `dark ? #20242C : #F3F9FF` | disabled-surface | `dark ? #20242C : #F3F9FF` | yes |
| `disabled-border` | color | `dark ? #3A4150 : #B0B7C3` | disabled-border | `dark ? #3A4150 : #B0B7C3` | yes |
| `disabled-fill` | color | `dark ? #4B5362 : #B0B7C3` | disabled-fill | `dark ? #4B5362 : #B0B7C3` | yes |
| `disabled-text` | color | `dark ? #77808F : #969A9E` | disabled-text | `dark ? #77808F : #969A9E` | yes |
| `status-neutral-surface` | color | `dark ? #2F3540 : #F3F9FF` | status-neutral-surface | `dark ? #2F3540 : #F3F9FF` | yes |
| `status-neutral-border` | color | `dark ? #9AA4B2 : #7A8594` | status-neutral-border | `dark ? #9AA4B2 : #7A8594` | yes |
| `status-neutral-text-on-surface` | color | `dark ? #F2F1EE : #101014` | status-neutral-text | `dark ? #F2F1EE : #101014` | yes |
| `status-success-surface` | color | `dark ? #0F2B27 : #E6F4F1` | status-success-surface | `dark ? #0F2B27 : #E6F4F1` | yes |
| `status-success-border` | color | `dark ? #3FD6B8 : #0A7563` | status-success-border | `dark ? #3FD6B8 : #0A7563` | yes |
| `status-success-text-on-surface` | color | `dark ? #3FD6B8 : #0A7563` | status-success-text | `dark ? #3FD6B8 : #0A7563` | yes |
| `status-warning-surface` | color | `dark ? #2F2612 : #FBF3E0` | status-warning-surface | `dark ? #2F2612 : #FBF3E0` | yes |
| `status-warning-border` | color | `dark ? #FFC14D : #8A5800` | status-warning-border | `dark ? #FFC14D : #8A5800` | yes |
| `status-warning-text-on-surface` | color | `dark ? #FFC14D : #8A5800` | status-warning-text | `dark ? #FFC14D : #8A5800` | yes |
| `status-danger-surface` | color | `dark ? #3A1816 : #FCEBEA` | status-danger-surface | `dark ? #3A1816 : #FCEBEA` | yes |
| `status-danger-border` | color | `dark ? #FF6259 : #B3261E` | status-danger-border | `dark ? #FF6259 : #B3261E` | yes |
| `status-danger-text-on-surface` | color | `dark ? #FF6259 : #B3261E` | status-danger-text | `dark ? #FF6259 : #B3261E` | yes |
| `status-info-surface` | color | `dark ? #18213D : #EAEFFA` | status-info-surface | `dark ? #18213D : #EAEFFA` | yes |
| `status-info-border` | color | `dark ? #9DB8FF : #3B5BA9` | status-info-border | `dark ? #9DB8FF : #3B5BA9` | yes |
| `status-info-text-on-surface` | color | `dark ? #9DB8FF : #3B5BA9` | status-info-text | `dark ? #9DB8FF : #3B5BA9` | yes |
| `shadow-overlay` | color | `dark ? #0000008C : #00000024` | shadow-overlay-color | `dark ? #0000008C : #00000024` | yes |
| `shadow-overlay-offset-y` | length | `8px` x zoom-factor | shadow-overlay-offset-y | `8px` | yes |
| `shadow-overlay-blur` | length | `24px` x zoom-factor | shadow-overlay-blur | `24px` | yes |
| `button-primary-bg` | color | `dark ? #4DA0FF : #1659C4` | component-button-primary-bg | `dark ? #4DA0FF : #1659C4` | yes |
| `button-primary-bg-hover` | color | `dark ? #8DBBFF : #164FA8` | component-button-primary-bg-hover | `dark ? #8DBBFF : #164FA8` | yes |
| `button-primary-bg-active` | color | `dark ? #5084DB : #123F80` | component-button-primary-bg-active | `dark ? #5084DB : #123F80` | yes |
| `button-primary-text` | color | `dark ? #0B0B0C : #FFFFFF` | component-button-primary-text | `dark ? #0B0B0C : #FFFFFF` | yes |
| `button-primary-border` | color | `dark ? #4DA0FF : #1659C4` | component-button-primary-border | `dark ? #4DA0FF : #1659C4` | yes |
| `button-secondary-bg` | color | `dark ? #1B1E24 : #FFFFFF` | component-button-secondary-bg | `dark ? #1B1E24 : #FFFFFF` | yes |
| `button-secondary-bg-hover` | color | `dark ? #0A0C10 : #F3F9FF` | component-button-secondary-bg-hover | `dark ? #0A0C10 : #F3F9FF` | yes |
| `button-secondary-bg-active` | color | `dark ? #2F3540 : #BDBFC4` | component-button-secondary-bg-active | `dark ? #2F3540 : #BDBFC4` | yes |
| `button-secondary-text` | color | `dark ? #F2F1EE : #101014` | component-button-secondary-text | `dark ? #F2F1EE : #101014` | yes |
| `button-secondary-border` | color | `dark ? #7F8A9B : #7A8594` | component-button-secondary-border | `dark ? #7F8A9B : #7A8594` | yes |
| `button-ghost-bg` | color | `dark ? #121316 : #F2F1EE` | component-button-ghost-bg | `dark ? #121316 : #F2F1EE` | yes |
| `button-ghost-bg-hover` | color | `dark ? #0A0C10 : #F3F9FF` | component-button-ghost-bg-hover | `dark ? #0A0C10 : #F3F9FF` | yes |
| `button-ghost-bg-active` | color | `dark ? #2F3540 : #BDBFC4` | component-button-ghost-bg-active | `dark ? #2F3540 : #BDBFC4` | yes |
| `button-ghost-text` | color | `dark ? #F2F1EE : #101014` | component-button-ghost-text | `dark ? #F2F1EE : #101014` | yes |
| `button-ghost-border` | color | `dark ? #121316 : #F2F1EE` | component-button-ghost-border | `dark ? #121316 : #F2F1EE` | yes |
| `button-danger-bg` | color | `dark ? #FF6259 : #B3261E` | component-button-danger-bg | `dark ? #FF6259 : #B3261E` | yes |
| `button-danger-bg-hover` | color | `dark ? #FF6259 : #B3261E` | component-button-danger-bg-hover | `dark ? #FF6259 : #B3261E` | yes |
| `button-danger-bg-active` | color | `dark ? #E8483F : #8A1D17` | component-button-danger-bg-active | `dark ? #E8483F : #8A1D17` | yes |
| `button-danger-text` | color | `dark ? #0B0B0C : #FFFFFF` | component-button-danger-text | `dark ? #0B0B0C : #FFFFFF` | yes |
| `button-danger-border` | color | `dark ? #FF6259 : #B3261E` | component-button-danger-border | `dark ? #FF6259 : #B3261E` | yes |
| `tooltip-bg` | color | `dark ? #1B1E24 : #FFFFFF` | component-tooltip-bg | `dark ? #1B1E24 : #FFFFFF` | yes |
| `tooltip-text` | color | `dark ? #F2F1EE : #101014` | component-tooltip-text | `dark ? #F2F1EE : #101014` | yes |
| `tooltip-border` | color | `dark ? #2F3540 : #BDBFC4` | component-tooltip-border | `dark ? #2F3540 : #BDBFC4` | yes |
| `panel-light-surface` | color | `dark ? #FFFFFF : #FFFFFF` | missing | `-` | missing |
| `panel-light-text` | color | `dark ? #101014 : #101014` | missing | `-` | missing |
| `panel-light-muted` | color | `dark ? #5E6B7C : #5E6B7C` | missing | `-` | missing |
| `radius-control` | length | `5px` x zoom-factor | radius-control | `6px` | **no** |
| `radius-surface` | length | `6px` x zoom-factor | radius-surface | `6px` | yes |
| `radius-pill` | length | `9999px` x zoom-factor | radius-pill | `9999px` | yes |
| `radius-none` | length | `0px` x zoom-factor | radius-none | `0px` | yes |
| `radius-r4` | length | `4px` x zoom-factor | radius-r4 | `4px` | yes |
| `radius-r6` | length | `6px` x zoom-factor | radius-r6 | `6px` | yes |
| `radius-r8` | length | `8px` x zoom-factor | radius-r8 | `8px` | yes |
| `space-0` | length | `0px` x zoom-factor | space-0 | `0px` | yes |
| `space-1` | length | `4px` x zoom-factor | space-1 | `4px` | yes |
| `space-2` | length | `8px` x zoom-factor | space-2 | `8px` | yes |
| `space-3` | length | `12px` x zoom-factor | space-3 | `12px` | yes |
| `space-4` | length | `16px` x zoom-factor | space-4 | `16px` | yes |
| `space-5` | length | `20px` x zoom-factor | space-5 | `20px` | yes |
| `space-6` | length | `24px` x zoom-factor | space-6 | `24px` | yes |
| `space-8` | length | `32px` x zoom-factor | space-8 | `32px` | yes |
| `space-10` | length | `40px` x zoom-factor | space-10 | `40px` | yes |
| `space-12` | length | `48px` x zoom-factor | space-12 | `48px` | yes |
| `size-control-height` | length | `34px` x zoom-factor | size-control-md | `34px` | yes |
| `size-control-height-sm` | length | `28px` x zoom-factor | size-control-sm | `28px` | yes |
| `size-control-height-lg` | length | `40px` x zoom-factor | size-control-lg | `40px` | yes |
| `size-badge-height` | length | `18px` x zoom-factor | size-badge-height | `18px` | yes |
| `size-badge-dot` | length | `8px` x zoom-factor | size-badge-dot | `8px` | yes |
| `size-kbd-height` | length | `20px` x zoom-factor | size-kbd-height | `20px` | yes |
| `size-tag-height` | length | `24px` x zoom-factor | size-tag-height | `24px` | yes |
| `size-avatar-sm` | length | `24px` x zoom-factor | size-avatar-sm | `24px` | yes |
| `size-avatar-md` | length | `32px` x zoom-factor | size-avatar-md | `32px` | yes |
| `size-avatar-lg` | length | `40px` x zoom-factor | size-avatar-lg | `40px` | yes |
| `size-presence-dot` | length | `10px` x zoom-factor | size-presence-dot | `10px` | yes |
| `size-progress-height` | length | `6px` x zoom-factor | size-progress-height | `6px` | yes |
| `size-skeleton-line` | length | `12px` x zoom-factor | size-skeleton-line | `12px` | yes |
| `size-sheet-sm` | length | `320px` x zoom-factor | size-sheet-sm | `320px` | yes |
| `size-sheet-md` | length | `400px` x zoom-factor | size-sheet-md | `400px` | yes |
| `size-sheet-lg` | length | `560px` x zoom-factor | size-sheet-lg | `560px` | yes |
| `size-dialog-sm` | length | `384px` x zoom-factor | size-dialog-sm | `384px` | yes |
| `size-dialog-md` | length | `480px` x zoom-factor | size-dialog-md | `480px` | yes |
| `size-dialog-lg` | length | `640px` x zoom-factor | size-dialog-lg | `640px` | yes |
| `size-sidebar-width` | length | `240px` x zoom-factor | size-sidebar-width | `240px` | yes |
| `size-rail-width` | length | `48px` x zoom-factor | size-rail-width | `48px` | yes |
| `size-phone-breakpoint` | length | `600px` x zoom-factor | size-breakpoint-phone | `640px` | **no** |
| `size-tab-bar-height` | length | `56px` x zoom-factor | size-tab-bar-height | `56px` | yes |
| `size-sidebar-row-height` | length | `36px` x zoom-factor | size-sidebar-row-height | `36px` | yes |
| `size-tree-row-height` | length | `30px` x zoom-factor | size-tree-row-height | `30px` | yes |
| `size-menu-row-height` | length | `28px` x zoom-factor | size-menu-row-height | `28px` | yes |
| `size-icon-small` | length | `16px` x zoom-factor | icon-small | `16px` | yes |
| `size-icon-medium` | length | `20px` x zoom-factor | icon-medium | `20px` | yes |
| `size-icon-large` | length | `24px` x zoom-factor | icon-large | `24px` | yes |
| `motion-fast` | duration | `reduced-motion ? 0ms : 120ms` | motion-fast | `reduced-motion ? 0ms : 120ms` | yes |
| `motion-base` | duration | `reduced-motion ? 0ms : 200ms` | motion-base | `reduced-motion ? 0ms : 200ms` | yes |
| `motion-slow` | duration | `reduced-motion ? 0ms : 320ms` | motion-slow | `reduced-motion ? 0ms : 320ms` | yes |
| `motion-loop` | duration | `reduced-motion ? 0ms : 1200ms` | motion-loop | `reduced-motion ? 0ms : 1200ms` | yes |
| `type-display-family` | string | `"Noto Sans"` | type-display-font-family | `"Noto Sans, sans-serif"` | **no** |
| `type-display-size` | length | `32px` x zoom-factor | type-display-font-size | `32px` | yes |
| `type-display-line-height` | length | `40px` x zoom-factor | type-display-line-height | `40px` | yes |
| `type-display-weight` | int | `600` | type-display-font-weight | `600` | yes |
| `type-title-family` | string | `"Noto Sans"` | type-title-font-family | `"Noto Sans, sans-serif"` | **no** |
| `type-title-size` | length | `26px` x zoom-factor | type-title-font-size | `26px` | yes |
| `type-title-line-height` | length | `34px` x zoom-factor | type-title-line-height | `34px` | yes |
| `type-title-weight` | int | `600` | type-title-font-weight | `600` | yes |
| `type-heading-family` | string | `"Noto Sans"` | type-heading-font-family | `"Noto Sans, sans-serif"` | **no** |
| `type-heading-size` | length | `22px` x zoom-factor | type-heading-font-size | `22px` | yes |
| `type-heading-line-height` | length | `30px` x zoom-factor | type-heading-line-height | `30px` | yes |
| `type-heading-weight` | int | `600` | type-heading-font-weight | `600` | yes |
| `type-subheading-family` | string | `"Noto Sans"` | type-subheading-font-family | `"Noto Sans, sans-serif"` | **no** |
| `type-subheading-size` | length | `16px` x zoom-factor | type-subheading-font-size | `16px` | yes |
| `type-subheading-line-height` | length | `24px` x zoom-factor | type-subheading-line-height | `24px` | yes |
| `type-subheading-weight` | int | `600` | type-subheading-font-weight | `600` | yes |
| `type-body-family` | string | `"Noto Sans"` | type-body-font-family | `"Noto Sans, sans-serif"` | **no** |
| `type-body-size` | length | `14px` x zoom-factor | type-body-font-size | `14px` | yes |
| `type-body-line-height` | length | `20px` x zoom-factor | type-body-line-height | `20px` | yes |
| `type-body-weight` | int | `400` | type-body-font-weight | `400` | yes |
| `type-body-strong-family` | string | `"Noto Sans"` | type-body-strong-font-family | `"Noto Sans, sans-serif"` | **no** |
| `type-body-strong-size` | length | `14px` x zoom-factor | type-body-strong-font-size | `14px` | yes |
| `type-body-strong-line-height` | length | `20px` x zoom-factor | type-body-strong-line-height | `20px` | yes |
| `type-body-strong-weight` | int | `600` | type-body-strong-font-weight | `600` | yes |
| `type-label-family` | string | `"Noto Sans"` | type-label-font-family | `"Noto Sans, sans-serif"` | **no** |
| `type-label-size` | length | `13.5px` x zoom-factor | type-label-font-size | `13.5px` | yes |
| `type-label-line-height` | length | `18px` x zoom-factor | type-label-line-height | `18px` | yes |
| `type-label-weight` | int | `500` | type-label-font-weight | `500` | yes |
| `type-caption-family` | string | `"Noto Sans"` | type-caption-font-family | `"Noto Sans, sans-serif"` | **no** |
| `type-caption-size` | length | `12px` x zoom-factor | type-caption-font-size | `12px` | yes |
| `type-caption-line-height` | length | `16px` x zoom-factor | type-caption-line-height | `16px` | yes |
| `type-caption-weight` | int | `400` | type-caption-font-weight | `400` | yes |
| `type-code-family` | string | `"JetBrains Mono"` | type-code-font-family | `"JetBrains Mono, monospace"` | **no** |
| `type-code-size` | length | `13px` x zoom-factor | type-code-font-size | `13px` | yes |
| `type-code-line-height` | length | `18px` x zoom-factor | type-code-line-height | `18px` | yes |
| `type-code-weight` | int | `400` | type-code-font-weight | `400` | yes |

### 1b. `Type` global (`ui/typography.slint`)

`Type` has nine `out property <TextStyle>` entries (`family`, `size`, `line-height`, `weight`) and one function `scaled(size)` (`typography.slint:13-15`: `return round(size / 1px * Tokens.text-factor * 2) * 0.5px;`). Each entry reads the `Tokens.type-<style>-*` properties, so its values are the `Tokens` rows above.

| Style | Space family | Design family | Space size / line height / weight | Design size / line height / weight | Equal |
|---|---|---|---|---|---|
| display | `"Noto Sans"` | `"Noto Sans, sans-serif"` | 32 px / 40 px / 600 | 32 px / 40 px / 600 | family no; rest yes |
| title | `"Noto Sans"` | `"Noto Sans, sans-serif"` | 26 / 34 / 600 | 26 / 34 / 600 | family no; rest yes |
| heading | `"Noto Sans"` | `"Noto Sans, sans-serif"` | 22 / 30 / 600 | 22 / 30 / 600 | family no; rest yes |
| subheading | `"Noto Sans"` | `"Noto Sans, sans-serif"` | 16 / 24 / 600 | 16 / 24 / 600 | family no; rest yes |
| body | `"Noto Sans"` | `"Noto Sans, sans-serif"` | 14 / 20 / 400 | 14 / 20 / 400 | family no; rest yes |
| body-strong | `"Noto Sans"` | `"Noto Sans, sans-serif"` | 14 / 20 / 600 | 14 / 20 / 600 | family no; rest yes |
| label | `"Noto Sans"` | `"Noto Sans, sans-serif"` | 13.5 / 18 / 500 | 13.5 / 18 / 500 | family no; rest yes |
| caption | `"Noto Sans"` | `"Noto Sans, sans-serif"` | 12 / 16 / 400 | 12 / 16 / 400 | family no; rest yes |
| code | `"JetBrains Mono"` | `"JetBrains Mono, monospace"` | 13 / 18 / 400 | 13 / 18 / 400 | family no; rest yes |

Space sizes and line heights are all `N px * zoom-factor` in the token global, and `Type` then passes them through `scaled()`. The design has no zoom and no text scale, so its sizes are plain px (section 2).

## 2. Runtime behaviour in Space that the generated file cannot express

The design's generated `Tokens` (`crate/ui/tokens.slint`) has `theme`, `dark`, `reduced-motion` and fixed token values. It has no zoom, no text scale and no computed length scaling. Space's behaviour, with quotes:

**Theme choice.** Space `tokens.slint:8,13,14`:
- `in-out property <ThemeChoice> theme: system;`
- `out property <bool> dark: theme == ThemeChoice.dark || (theme == ThemeChoice.system && Palette.color-scheme == ColorScheme.dark);`

The design generates the same `dark` expression (`crate/ui/tokens.slint:8`) and the same default (`:7`). Rust side: `slint_ui/theme.rs:35-46` maps `ThemeMode` to `ThemeChoice` and calls `set_theme`. `app_shell.rs:1598` and `:3132` call `slint_theme::set_mode`. `ThemeMode::resolve` (`src/theme.rs:17-23`) and `slint_ui/theme.rs:49-51` (`is_dark`) are not read by the shipped app. `is_dark` is called only from the gallery harness (`apps/gallery/src/harness.rs:301,515,655`).

**Dark flag.** `Tokens.dark` is read in Slint only through the colour expressions. Rust reads it only in the gallery (above). No runtime work is needed beyond the expression.

**Reduced motion.** Space `tokens.slint:15-16`: "Manual reduced-motion flag. Set from Rust; the OS setting is not readable from Slint". Lines 172-175 gate every duration: `out property <duration> motion-fast: reduced-motion ? 0ms : 120ms;` (base 200 ms, slow 320 ms, loop 1200 ms). The design has the same gate (`crate/ui/tokens.slint:9` and its four duration rows). Rust setter: `slint_ui/theme.rs:53-56`. Callers in the shipped app: none. Calls exist only in `apps/gallery/src/harness.rs:184`, `crates/rimelight_ui/tests/ui/display.rs:99,107` and `tests/ui/sheet.rs:149,155`. The Rust `Motion` struct (`src/motion.rs:19-32`) and `display::motion_duration_ms` (`src/slint_ui/display.rs:65-66`) are used only in tests. Net effect: the shipped app never sets reduced motion and never reads the OS setting.

**Zoom.** Space `tokens.slint:20-22`: "Interface zoom in percent (the status bar slider, 50 to 150, Ctrl+plus and Ctrl+minus). Multiplies every length." Declarations: `in-out property <int> zoom-percent: 100;` and `out property <float> zoom-factor: zoom-percent * 1.0 / 100.0;`. Every length is written `Npx * zoom-factor`, from `codegen.rs:34-36` (`format!("{value}px * zoom-factor")`). That covers 66 length tokens: shadow offset and blur (`tokens.slint:84-85`), radii (`:120-126`), spacing (`:129-138`), sizes (`:141-169`, including the phone breakpoint at `:162`), and type sizes and line heights (`:179-211`).

The setter chain in `ui/app-shell.slint`:
- `:681` `in-out property <float> zoom-percent: 100;`
- `:687` `in-out property <float> zoom-applied: 100;`
- `:688-689` `changed zoom-applied => { Tokens.zoom-percent = round(root.zoom-applied); }`
- `:1177` `root.zoom-percent = min(150, root.zoom-percent + 10);` and `:1183` `max(50, root.zoom-percent - 10)` (keyboard steps; the bounds 50 and 150 and the step 10 are literals here, not tokens)
- `:2524` `zoom-percent <=> root.zoom-percent;` (status bar slider)

Rust sets `zoom-applied` from `app_shell.rs:1707-1711` (`on_zoom_changed` calls `set_zoom_applied`) and `shell_capture.rs:355-358`. Space's Slint also multiplies literal lengths by zoom in 31 places: `components/sidebar.slint` (27, all in `SidebarSize`, lines 91-119), `components/slider.slint:40,83,85` and `components/status-bar.slint:359`.

**Phone breakpoint under zoom.** `ui/app-shell.slint:229` and `:234`: `root.phone = root.width < Tokens.size-phone-breakpoint;`. The window width is in unscaled logical px, but the breakpoint is 600 px times the zoom factor, so at 150 per cent zoom the phone layout starts below 900 px. The 640 px design value keeps this rule, so the zoom multiplier must survive the switch.

**Text scale.** Space `tokens.slint:17-19`: "Text scale in percent: 100, 125, 150 or 200. Multiplies every type size and line height." `text-factor: text-scale-percent * 1.0 / 100.0`. `typography.slint:12-15` applies it through `scaled()`, which rounds to half a pixel: `round(size / 1px * Tokens.text-factor * 2) * 0.5px`. The Rust formula is `src/tokens/typography.rs:183-185` (`(px * scale.factor() * 2.0).round() / 2.0`). Rust setter: `slint_ui/theme.rs:58-61`. Callers: only the gallery (`apps/gallery/src/harness.rs:151`, and its own `set_text_scale` at `:291-292`). The shipped app never leaves text scale at 100. The design has no equivalent of `text-factor`, `scaled()` or the half-pixel rounding.

**Other computed values.** Shadow colour `dark ? #0000008C : #00000024` (`tokens.slint:83`; design `shadow-overlay-color`, equal). Shadow offset and blur are zoomed (`tokens.slint:84-85`). The design has `shadow-overlay-offset-y` and `shadow-overlay-blur` as plain px.

**Gallery-only writes.** `ui/gallery.slint:167` sets `Tokens.theme` and `:172` sets `Tokens.text-scale-percent` from the gallery toolbar. Those are not shipped behaviour.


## 3. Missing tokens, grouped

### 3a. Values that belong in `tokens.json` (proposed paths and values)

- `color.extension.panel-light.surface`: light `#FFFFFF`, dark `#FFFFFF`. Space `tokens.slint:115` (`panel-light-surface`). Fixed light panel for the vendor AboutSlint widget.
- `color.extension.panel-light.text`: light `#101014`, dark `#101014`. Space `tokens.slint:116`.
- `color.extension.panel-light.muted`: light `#5E6B7C`, dark `#5E6B7C`. Space `tokens.slint:117`.
- `extension.zoom.step`: number `10`. Space hard-codes the step at `app-shell.slint:1177,1183`. `extension.zoom.default`, `.min` (50) and `.max` (150) already exist in `tokens.json`, and Space's literals match them. Use those.
- Focus width: `focus.width` (2 px) already exists. Space uses literal `2px` for focus borders in several places (for example `ui/components/issue-card.slint:56`, `number-input.slint:108`, `quick-action.slint:58`, `view-switcher.slint:70`). Those should read `focus.width`.

Nothing else needs a new JSON value. The owner-approved radius (6) and breakpoint (640) are already in `tokens.json` and `crate/ui/tokens.slint`.

Generator problems (not JSON values): the Slint output needs a bare family name (`"Noto Sans"`), not the CSS-style list (`"Noto Sans, sans-serif"`), and it needs the zoom and text-scale hooks in section 2.

### 3b. Component-specific values that should become component tokens

Space `ui/components/sidebar.slint:91-119`, `SidebarSize` (27 zoom-scaled lengths, all `N px * Tokens.zoom-factor`). Proposed `component.sidebar.*`:

| Space name | Value (px) | Space line |
|---|---|---|
| header-height | 36 | 91 |
| row-height | 32 | 93 |
| pad-x | 10 | 94 |
| gap | 9 | 95 |
| icon | 17 | 96 |
| sub-icon | 16 | 97 |
| sub-gap | 7 | 98 |
| child-indent | 19 | 100 |
| chevron | 15 | 101 |
| heading-top | 12 | 102 |
| heading-line | 16 | 103 |
| heading-font | 12 | 104 |
| heading-tracking | 0.72 | 105 |
| sub-font | 14 | 106 |
| row-font | 15 | 107 |
| count-font | 12 | 108 |
| count-height | 19 | 109 |
| count-pad-x | 6 | 110 |
| count-gap | 6 | 111 |
| status-dot | 8 | 112 |
| status-gap | 4 | 113 |
| tile | 17 | 114 |
| avatar | 18 | 115 |
| lock | 11 | 116 |
| people-icon | 12 | 117 |
| people-gap | 3 | 118 |
| indicator | 3 | 119 |

Other zoom-scaled component literals:
- `ui/components/slider.slint:40` `label-w` 40 px; `:83` `min-width` 120 px; `:85` `preferred-width` 240 px. Proposed `component.slider.label-width`, `.min-width`, `.preferred-width`.
- `ui/components/status-bar.slint:359` `width` 160 px. Proposed `component.status-bar.item-width`.
- Rust `src/slint_ui/tile_drop.rs:17-21`: `STRIP_HEIGHT` = `CONTROL_HEIGHT` (34), `STRIP_TAB_WIDTH` = `CONTROL_HEIGHT * 4` (136), `STRIP_PINNED_WIDTH` = `CONTROL_HEIGHT` (34). Proposed `component.tile-strip.height` 34, `.tab-width` 136, `.pinned-width` 34. The Rust side reads them through the design crate.
- Design-only component values already in `tokens.json` (`component.toolbar.height` 44, `component.action-row.height` 44, `component.status-bar.height` 36, `component.recent-list-row.height` 48, `component.empty-state.top-section` 300). Not checked whether Space has matching literals. Verify before deleting or using.
- Not inventoried: about 1,450 non-comment Slint lines with literal `px` that do not scale with zoom and have no token. They belong to pages and components, so each needs a per-component review. This is outside the ST2.2 switch, but it means components cannot all read tokens after the switch.

### 3c. Things that must stay app-side

- Theme choice from user settings: `ThemeMode` (`src/theme.rs:6-24`) and `slint_ui/theme.rs:35-41` (`choice`), `:44-46` (`set_mode`). Persisted state stays in Space.
- Runtime setters and their state: `reduced-motion` (`slint_ui/theme.rs:53-56`), `text-scale-percent` (`:58-61`), `zoom-percent` / `zoom-applied` (`app-shell.slint:681-689`, `app_shell.rs:1707-1711`, `shell_capture.rs:355-358`). The state stays app-side, but the multiplication must live in the generated global (generator change, section 7 step 1).
- The `Type` global and `TextStyle` struct (`ui/typography.slint`). Keep it app-side or move it into the design generator. It reads design values.
- `display::motion_duration_ms` (`src/slint_ui/display.rs:65`) and `Motion::resolve` (`src/motion.rs:21`). Both are test-only today. Delete or keep app-side.
- Gallery harness (`apps/gallery/src/harness.rs`) and its `TextScale` use.

## 4. Proposed rename map (Space name to design name)

Only properties that exist in both under different names. Properties with identical names keep their names and need no entry. `radius-control` keeps its name and changes value (5 to 6). The `type-*` map covers all 27 names (family, size, weight).

```json
{
  "button-primary-bg": "component-button-primary-bg",
  "button-primary-bg-hover": "component-button-primary-bg-hover",
  "button-primary-bg-active": "component-button-primary-bg-active",
  "button-primary-text": "component-button-primary-text",
  "button-primary-border": "component-button-primary-border",
  "button-secondary-bg": "component-button-secondary-bg",
  "button-secondary-bg-hover": "component-button-secondary-bg-hover",
  "button-secondary-bg-active": "component-button-secondary-bg-active",
  "button-secondary-text": "component-button-secondary-text",
  "button-secondary-border": "component-button-secondary-border",
  "button-ghost-bg": "component-button-ghost-bg",
  "button-ghost-bg-hover": "component-button-ghost-bg-hover",
  "button-ghost-bg-active": "component-button-ghost-bg-active",
  "button-ghost-text": "component-button-ghost-text",
  "button-ghost-border": "component-button-ghost-border",
  "button-danger-bg": "component-button-danger-bg",
  "button-danger-bg-hover": "component-button-danger-bg-hover",
  "button-danger-bg-active": "component-button-danger-bg-active",
  "button-danger-text": "component-button-danger-text",
  "button-danger-border": "component-button-danger-border",
  "tooltip-bg": "component-tooltip-bg",
  "tooltip-text": "component-tooltip-text",
  "tooltip-border": "component-tooltip-border",
  "rail-bg": "extension-brand-rail-bg",
  "rail-icon-inactive": "extension-brand-rail-icon-inactive",
  "rail-icon-active": "extension-brand-rail-icon-active",
  "sidebar-bg": "extension-brand-sidebar-bg",
  "sidebar-text": "extension-brand-sidebar-text",
  "sidebar-text-muted": "extension-brand-sidebar-text-muted",
  "sidebar-icon": "extension-brand-sidebar-icon",
  "sidebar-hover-bg": "extension-brand-sidebar-hover-bg",
  "sidebar-selected-bg": "extension-brand-sidebar-selected-bg",
  "sidebar-selected-text": "extension-brand-sidebar-selected-text",
  "sidebar-selected-indicator": "extension-brand-sidebar-selected-indicator",
  "sidebar-link": "extension-brand-sidebar-link",
  "sidebar-divider": "extension-brand-sidebar-divider",
  "shadow-overlay": "shadow-overlay-color",
  "size-control-height": "size-control-md",
  "size-control-height-sm": "size-control-sm",
  "size-control-height-lg": "size-control-lg",
  "size-icon-small": "icon-small",
  "size-icon-medium": "icon-medium",
  "size-icon-large": "icon-large",
  "size-phone-breakpoint": "size-breakpoint-phone",
  "status-success-text-on-surface": "status-success-text",
  "status-warning-text-on-surface": "status-warning-text",
  "status-danger-text-on-surface": "status-danger-text",
  "status-info-text-on-surface": "status-info-text",
  "status-neutral-text-on-surface": "status-neutral-text",
  "type-display-family": "type-display-font-family",
  "type-display-size": "type-display-font-size",
  "type-display-weight": "type-display-font-weight",
  "type-title-family": "type-title-font-family",
  "type-title-size": "type-title-font-size",
  "type-title-weight": "type-title-font-weight",
  "type-heading-family": "type-heading-font-family",
  "type-heading-size": "type-heading-font-size",
  "type-heading-weight": "type-heading-font-weight",
  "type-subheading-family": "type-subheading-font-family",
  "type-subheading-size": "type-subheading-font-size",
  "type-subheading-weight": "type-subheading-font-weight",
  "type-body-family": "type-body-font-family",
  "type-body-size": "type-body-font-size",
  "type-body-weight": "type-body-font-weight",
  "type-body-strong-family": "type-body-strong-font-family",
  "type-body-strong-size": "type-body-strong-font-size",
  "type-body-strong-weight": "type-body-strong-font-weight",
  "type-label-family": "type-label-font-family",
  "type-label-size": "type-label-font-size",
  "type-label-weight": "type-label-font-weight",
  "type-caption-family": "type-caption-font-family",
  "type-caption-size": "type-caption-font-size",
  "type-caption-weight": "type-caption-font-weight",
  "type-code-family": "type-code-font-family",
  "type-code-size": "type-code-font-size",
  "type-code-weight": "type-code-font-weight"
}
```

That is 76 entries. The `type-*-line-height` names and `motion-*`, `space-*`, `radius-*`, `surface-*`, `text-*`, `accent-*`, `focus-ring`, `border-*`, `disabled-*`, `selection`, `scrim`, `status-*` (base) names are identical in both and do not need entries.

Use counts across `crates/rimelight_ui/ui/**/*.slint` (from `rg -o 'Tokens\.[a-zA-Z0-9_-]+'`, counted over all `.slint` files):

```json
{
  "button-primary-bg": 2, "button-primary-bg-hover": 2, "button-primary-bg-active": 2, "button-primary-text": 2, "button-primary-border": 1,
  "button-secondary-bg": 2, "button-secondary-bg-hover": 1, "button-secondary-bg-active": 1, "button-secondary-text": 1, "button-secondary-border": 1,
  "button-ghost-bg": 0, "button-ghost-bg-hover": 11, "button-ghost-bg-active": 3, "button-ghost-text": 1, "button-ghost-border": 0,
  "button-danger-bg": 1, "button-danger-bg-hover": 1, "button-danger-bg-active": 1, "button-danger-text": 1, "button-danger-border": 1,
  "tooltip-bg": 1, "tooltip-text": 1, "tooltip-border": 1,
  "rail-bg": 4, "rail-icon-inactive": 3, "rail-icon-active": 3,
  "sidebar-bg": 17, "sidebar-text": 21, "sidebar-text-muted": 13, "sidebar-icon": 1, "sidebar-hover-bg": 10, "sidebar-selected-bg": 6, "sidebar-selected-text": 5, "sidebar-selected-indicator": 3, "sidebar-link": 4, "sidebar-divider": 7,
  "shadow-overlay": 14,
  "size-control-height": 86, "size-control-height-sm": 28, "size-control-height-lg": 9,
  "size-icon-small": 62, "size-icon-medium": 10, "size-icon-large": 6,
  "size-phone-breakpoint": 2,
  "status-success-text-on-surface": 3, "status-warning-text-on-surface": 2, "status-danger-text-on-surface": 13, "status-info-text-on-surface": 2, "status-neutral-text-on-surface": 2,
  "type-display-family": 1, "type-display-size": 1, "type-display-weight": 1,
  "type-title-family": 1, "type-title-size": 1, "type-title-weight": 1,
  "type-heading-family": 1, "type-heading-size": 1, "type-heading-weight": 1,
  "type-subheading-family": 2, "type-subheading-size": 2, "type-subheading-weight": 2,
  "type-body-family": 5, "type-body-size": 4, "type-body-weight": 4,
  "type-body-strong-family": 1, "type-body-strong-size": 1, "type-body-strong-weight": 1,
  "type-label-family": 2, "type-label-size": 2, "type-label-weight": 2,
  "type-caption-family": 11, "type-caption-size": 11, "type-caption-weight": 11,
  "type-code-family": 4, "type-code-size": 2, "type-code-weight": 2
}
```

Not in the rename map but affected: `radius-control` has 39 uses (value change 5 to 6, owner-approved). `size-phone-breakpoint` has 2 uses. `Tokens.zoom-factor` has 31 uses and `Tokens.text-factor` 25 uses; both disappear unless the generator provides them (gap 1). The 9 `Type` style names (display ... code) are not renamed; the `Type` global itself stays.

## 5. Rust uses of Space's token modules

| Space file:line | What it reads | rimelight_design equivalent | Status |
|---|---|---|---|
| `src/motion.rs:4,24-29,52` | `primitive::motion` DURATION_FAST/BASE/SLOW/LOOP_MS, EASE_STANDARD, EASE_ENTER. `Motion::resolve` is test-only. | `tokens::motion` (`crate/src/generated.rs:743-750`): FAST_MS, BASE_MS, SLOW_MS, LOOP_MS, EASE_STANDARD, EASE_ENTER | Equal values. Rename. |
| `src/icon.rs:139-143` | `size::ICON_SMALL/MEDIUM/LARGE` | `tokens::icon` (`generated.rs:692-696`): SMALL, MEDIUM, LARGE | Equal values. Rename. |
| `src/slint_ui/tile_drop.rs:14,17,19,21` | `size::CONTROL_HEIGHT` (34), used as strip height, tab width (x4) and pinned width | `tokens::size::CONTROL_MD` (`generated.rs:661-665`) | Value equal. Strip constants are missing (section 3b). |
| `src/tokens/semantic.rs:184-189` (`RADIUS`) | `radius::R5`, `R6`, `PILL`, `R0` | `tokens::radius` (`generated.rs:651-657`): NONE, R4, R6, R8, CONTROL (6), SURFACE (6), PILL | Control radius changes 5 to 6 (owner). |
| `src/tokens/semantic.rs:191-461` (`Semantic`, `light()`, `dark()`, `colours()`) | every semantic colour | `tokens::color::{light,dark}` (`generated.rs:285,387`) and `tokens::ROLES` (`:773`) | Values equal. No `Semantic` struct; design exposes flat constants. Names differ (section 4). |
| `src/tokens/component.rs` (`Component::from_semantic`) | button, input, tab, table row and tooltip aliases | `tokens::component::{light,dark}` (`generated.rs:490-580`), e.g. `BUTTON_PRIMARY_BG` | Values equal. Rename to `COMPONENT_*` style per design. |
| `src/tokens/primitive.rs` (`Rgba`, colour constants, `ALL`) | colour primitives | `Rgba` (`generated.rs:5`) and `tokens::ramp::{light,dark}` (`:16-150`) | `Rgba` equal. Primitive names differ and design ramp values differ (ramp-neutral-50 is `#F9FAFB` in the design, `#F2F1EE` is Space's background). Not used by Space's `Tokens`. |
| `src/tokens/primitive.rs` (`space`) | `space::S0..S12` | `tokens::space::N_0..N_12` (`generated.rs:638-649`) | Values equal. Rename. |
| `src/tokens/primitive.rs` (`size`) | CONTROL_*, PHONE_BREAKPOINT (600), SIDEBAR/RAIL, TREE/MENU rows, icon sizes | `tokens::size` (`generated.rs:661-690`) | Equal except `PHONE_BREAKPOINT` (600 to `BREAKPOINT_PHONE` 640, owner). Renamed names. |
| `src/tokens/primitive.rs` (`ELEVATION_OVERLAY_LIGHT/DARK`, `:334-345`) | offset 8, blur 24, alpha 36 / 140 | `tokens::shadow` (`generated.rs:752-765`) | Equal. Offsets are not zoomed in the design (gap 1). |
| `src/tokens/primitive.rs` (`radius`, `motion` modules) | see rows above | see rows above | |
| `src/tokens/typography.rs` (`TextScale`, `style()`, `scaled()`, `TextStyleName`, `Family`) | text scale and per-style values | `tokens::typography` (`generated.rs:704-742`): constants only | Missing: `TextScale`, `scaled()`, factors. `Family` becomes the family string. |
| `src/tokens/contrast.rs` (`checked_pairs`, `contrast_ratio`, `EXEMPT`, `TEXT_MIN`, `NON_TEXT_MIN`) | contrast pairs | None in Rust. `generate.ts` contrast section (around lines 521-590) and `tokens.json` `$extensions.rimelight.contrast` | Missing as a Rust API. Covered by the generator. |
| `src/slint_ui/theme.rs:11,35-41` | `TextScale`; `ThemeMode` to `ThemeChoice` | none (app-side) | Keep app-side. |
| `src/slint_ui/theme.rs:44-61` | `set_theme`, `get_dark`, `set_reduced_motion`, `set_text_scale_percent` | none (Slint props exist in design `Tokens`) | Keep app-side. |
| `src/theme.rs:4,30-31,45-56,70-74` | `Semantic`, `Component`, `Theme::light/dark` | none; `Theme` is not read by production code | Delete with `Semantic` or keep as app-side. |
| `src/slint_ui/codegen.rs:16-19,34-317` | everything above, written to `tokens.slint` | `generate.ts` writes `crate/ui/tokens.slint` | Replaced by the generator. |
| `src/slint_ui/display.rs:65-66` | `motion_duration_ms` (not a token module) | none | Keep app-side. Test-only. |
| `apps/gallery/src/harness.rs:15,94,151,263-266,291-297,301,341-344,357` and `apps/gallery/src/main.rs:34,101` | `TextScale`; sets `text-scale-percent`, `reduced-motion`, `theme`; reads `is_dark` | none | Keep app-side (gallery). |
| `tests/ui/display.rs:98-107`, `tests/ui/sheet.rs:149-155` | sets `Tokens.reduced-motion` | none | Keep app-side. |

## 6. Contrast tests

Space checks (`src/tokens/contrast.rs`): `checked_pairs` (lines 134-352) builds 91 pairs. The thresholds are `TEXT_MIN` 4.5 and `NON_TEXT_MIN` 3.0 (lines 12-14), the same as the design. By group: text on the four surfaces (12); on-accent text on the three fills (3); status text on base and raised (8); sidebar text and link (6); sidebar and rail icons and indicator (4); focus, strong border, track and accent on the four surfaces (16); checkbox mark (1); switch off and on (2); slider thumb (1); feedback tones (15); buttons (12); input (4); tabs (3); table rows (3); tooltip (1).

The design `tokens.json` has 98 pairs (`$extensions.rimelight.contrast.pairs`). Reconciled by name and value:
- All 91 Space pairs have a design pair with the same foreground, background and minimum. Four Space pairs have no design name but an identical value pair in the design list: `checkbox.mark` on `accent.default` and `switch.thumb.on` on `accent.default` (both `text.on-accent` on `accent.default`), `switch.thumb.off` (`surface.raised` on `border.strong`), and `slider.thumb` (`surface.base` on `accent.default`, the slider track is `accent.default`).
- The design adds 7 pairs that Space does not check: `text.on-accent` on `status.success`, `status.warning`, `status.danger`, `status.info` (4 at 4.5); `component.avatar.initials` on `border.strong` (4.5); `border.input` on `surface.base` and `surface.raised` (3.0 each).

So the answer to "which pairs Space checks that tokens.json does not cover" is none. The design covers a superset.

Exemptions: Space `EXEMPT` (`contrast.rs:17-50`) lists 9 names: `text.faint`, `border.subtle`, `sidebar.divider`, `selection`, `scrim`, `disabled.text`, `disabled.surface`, `disabled.border`, `disabled.fill`. The design `exempt` list (`tokens.json`) has the same 9, with `extension.brand.sidebar.divider` in place of `sidebar.divider`.

Space tests (`contrast.rs:395-511`):
- `every_pair_meets_its_minimum_in_both_themes`: asserts more than 40 pairs in each theme and no failures. Design equivalent: the generator checks every pair in both themes.
- `pairs_cover_the_required_text_roles`: asserts that 19 named text roles and 4 named non-text roles are present. I did not find an equivalent role-list assertion in `generate.ts`. Grep for it before relying on it.
- `exempt_list_names_only_tokens_that_exist`: design equivalent is the generator's exempt check (a missing role fails the build).
- `black_on_white_is_21_to_1`, `identical_colours_are_1_to_1`, `known_grey_on_white`, `luminance_endpoints`: test the WCAG function only. Not token coverage. The design needs its own equivalent if the Rust function is removed.

Freshness: Space `generated_tokens_are_up_to_date` (`codegen.rs:375-389`) compares the committed file with `tokens_slint()`. The design equivalent is `node generate.ts --check` (`generate.ts:5`), which exits 1 on any difference or contrast failure.

## 7. Recommended order of changes

1. Generator first (design package, no Space change):
   a. Emit bare family names in Slint `type-*-font-family` (`"Noto Sans"`, `"JetBrains Mono"`), not `"Noto Sans, sans-serif"`. Verify in the gallery that Slint resolves them.
   b. Add an opt-in zoom mode to `generate.ts`: `in-out property <int> zoom-percent: 100`, `out property <float> zoom-factor`, and `N px * zoom-factor` on every length (match `codegen.rs:34-36`). Keep `reduced-motion` gating.
   c. Add `in-out property <int> text-scale-percent` and `out property <float> text-factor`, and a `scaled()` function for `Type` with the same half-pixel rounding (`typography.slint:12-15`), or document that `Type` stays app-side.
   d. Add to `tokens.json`: `color.extension.panel-light.{surface,text,muted}`, `extension.zoom.step` (10), and the `component.sidebar.*`, `component.slider.*`, `component.status-bar.item-width`, `component.tile-strip.*` values from section 3b.
   e. Confirm `focus.width` is emitted to Slint and Rust (it is in `tokens.json`).
   f. Run `node generate.ts` and `--check`. Confirm the diff against Space's current `tokens.slint` is only the owner-approved radius and breakpoint, the font-family change, and the added names.
2. Space: add the `rimelight_design` dependency (crate `packages/design/crate`), and point the Slint import at its `ui/tokens.slint`. Keep `Tokens`, `ThemeChoice`, `Type` and `TextScale` names where Space code uses them, or update them in one pass.
3. Space: apply the rename map (section 4) to the 2,600 Slint references. Run it as one scripted pass and check with `rg` that no old name remains. Update `radius-control` expectations (39 uses).
4. Space: replace the Rust uses in section 5 with design constants. Delete `src/tokens/*`, `src/slint_ui/codegen.rs`, `ui/generated/tokens.slint` and the freshness test. Keep the app-side theme, zoom and text-scale glue (`slint_ui/theme.rs`, `app_shell.rs` zoom handling, `ThemeMode`).
5. Space: move the 31 zoom-scaled literal lengths (`sidebar.slint`, `slider.slint`, `status-bar.slint`) and the strip constants in `tile_drop.rs` to the new component tokens.
6. Verify: run the gallery screenshot matrix (themes, 100/125/150/200 text scale, zoom 50/100/150, reduced motion on and off) before and after the switch. Expected differences: radius-control 5 to 6, phone breakpoint 600 to 640 (at 100 per cent zoom and at each zoom). Any other pixel difference is a regression. Run the design `--check` and Space's contrast tests on the new values.

## Biggest gaps

1. Runtime scaling does not exist in the design. Space's 66 length tokens, its 18 type size and line-height tokens, and its 31 literal zoom sites all multiply by `zoom-factor`, and `Type` multiplies by `text-factor` (`tokens.slint:17-22`, `typography.slint:12-15`). The design's generated Slint has none of this. Without a generator change, zoom stops working in every window at once. Text scale is gallery-only in Space today, but the generator change is still needed for the gallery to keep its matrix.
2. Font family strings differ and are the likely visible break. The design emits `"Noto Sans, sans-serif"` (and `"JetBrains Mono, monospace"`) for all nine `Type` styles. Space uses the bare family name. `Type` supplies family to about 1,046 text sites. If Slint does not resolve the list form, every text site falls back to the default font. This must be fixed in the generator and checked in the gallery.
3. Components are not tokenised. About 1,450 non-comment Slint lines use literal px that no token or zoom covers, and the design's component layer does not contain the sidebar, slider, status-bar or tile-strip values Space uses (27 `SidebarSize` lengths alone). Without component tokens, the switch removes nothing from those components and the design's component layer is not used for them.

Other findings to act on: the shipped Space app never sets reduced motion or text scale (only the gallery and tests do), and `Motion`, `Theme` and `display::motion_duration_ms` are test-only. The Slint `.slint` file `ui/motion.slint` mentioned in `codegen.rs:10-11` does not exist; easings are written inline (`ease-in-out`, `ease-out`). The design's `motion-ease-standard` and `motion-ease-enter` have no Space consumer.
