# Ruphish admin UI design system

This guide defines Ruphish's visual language and how to apply it when designing or extending the admin interface. Use it to choose colors, typography, layouts, components and interaction patterns consistently.

## Design principles

Ruphish is a privacy-respecting security simulation tool. Its interface should make operational work understandable: what the operator is configuring, what will happen next, what has completed and what needs attention.

- **Clear hierarchy:** make the page purpose, main action and important state easy to identify.
- **Consistent behavior:** equivalent tasks use the same controls, language and interaction patterns across features.
- **Calm presentation:** use spacing, typography and quiet borders to organize information. Components use uniform borders on all sides or no border; never use a single-edge accent border. Reserve strong emphasis for actions, status and exceptions.
- **Visible outcomes:** distinguish pending work, successful results, failures and unavailable information.
- **Preserved context:** keep entered values, selections and navigation context through ordinary interactions and recoverable failures.
- **Accessible operation:** support keyboard, pointer and touch input, with readable labels and visible focus.

## Theme and color

The theme combines a deep teal navigation area, pale teal-tinted application background and white content surfaces. Teal is the primary action color. Red signals destructive actions and errors; amber signals caution; blue signals information or active work.

| Role | Token | Color | Use |
| --- | --- | --- | --- |
| Main text | `--ink` | `#13302c` | Headings, labels and body text |
| Secondary text | `--muted` | `#5b716e` | Supporting descriptions and metadata |
| Application background | `--canvas` | `#f4f7f6` | Space surrounding content surfaces |
| Content surface | `--paper` | `#ffffff` | Forms, cards, tables and overlays |
| Quiet separator | `--line` | `#dde5e3` | Surface boundaries and chart grids |
| Control border | `--line-strong` | `#c3d0cd` | Inputs and secondary controls |
| Neutral hover | `--hover` | `#eef4f2` | Hover feedback on neutral surfaces |
| Primary action | `--teal-600` | `#0b6b63` | Primary buttons, links and focus |
| Primary hover | `--teal-700` | `#0a534d` | Hover on primary actions |
| Positive or selected surface | `--teal-50`, `--teal-100` | `#e9f5f3`, `#cfe9e5` | Positive feedback and selected controls |
| Navigation foundation | `--teal-ink` | `#0c2f2b` | Dark sidebar surface |
| Error or destructive action | `--danger`, `--danger-soft` | `#a4303c`, `#fcf0f1` | Errors, destructive confirmations and their supporting surfaces |
| Warning | `--amber`, `--amber-soft`, `--warning-ink` | `#b4791f`, `#fbf3e1`, `#513d0e` | Caution, warning surfaces and readable warning text |
| Information | `--info` | `#23609a` | Informational or active states |

Choose colors by meaning. A secondary action does not become destructive because it dismisses a form. Chart colors identify categories and do not automatically imply success or failure.

Pair status colors with text, icons or another visible distinction. Use the navigation color variants for text, hover, selection and focus on the dark sidebar; colors intended for white surfaces may not remain legible there.

## Typography

Use **IBM Plex Sans** for navigation, headings, labels, names and prose. Use **IBM Plex Mono** for metric readouts, chart values, timestamps, code and secret values. Numeric comparisons benefit from tabular figures. Provide suitable system fallbacks.

| Purpose | Size | Typical weight |
| --- | --- | --- |
| Fine supporting text | 12px | Regular |
| Labels and compact supporting text | 13px | Regular or semibold |
| Body text and standard controls | 15px | Regular or medium |
| Section heading | 19px | Semibold |
| Page heading | 28px | Semibold |
| Prominent metric | 40px | Medium |

Use weights 400, 500, 600 and 700 deliberately. Body text uses approximately 1.5 line height; longer prose can use more breathing room. Headings use tighter line height and sentence case.

Choose text width for its purpose. Long reading passages may benefit from a narrower measure. Operational explanations, tables, charts and form sections should use the space their content needs; do not impose a universal prose-width limit on an entire interface.

## Spacing, shape and elevation

Use the shared spacing scale: **4, 8, 12, 16, 24 and 32px**. Keep related controls close together and give separate sections visibly more space. A section heading, its explanation and its fields should read as one group.

| Element | Guideline |
| --- | --- |
| Standard control | 36px minimum height |
| Compact row action | 30px minimum height, with adequate interactive space |
| Control corners | 6px radius |
| Content and overlay corners | 10px radius |
| Status pills and active navigation | Fully rounded |
| Resting surfaces | Quiet borders |
| Floating panels and dialogs | Restrained shadow to establish elevation |
| Keyboard focus | Clear 2px treatment with contrast against the surrounding surface |

Align adjacent buttons with the input or select itself, rather than the center of a combined label-and-control group. Match control heights. Use baseline alignment for text, top alignment for uneven content, and centering within controls and icons.

## Page layout and navigation

Give each page a clear title, optional supporting context and a small set of relevant actions. The main content should have a predictable reading order: context, controls, results and supporting details.

Group sidebar destinations by purpose. Pair navigation icons with readable labels and use a filled active treatment to identify the current destination. Navigation should remain stable as the main content changes.

Use the available page width for resource forms, tables, charts and operational summaries. Narrow focused tasks where that improves comprehension. Cards group related information; use them to establish meaningful boundaries rather than enclosing every individual element.

For forms, place cancellation at the bottom left and the primary save or create action at the bottom right. For page-level actions, use a consistent heading area. Keep an action next to the content it affects when that relationship is more useful than a global placement.

Responsive layouts follow the content. Reduce grid columns when fields become cramped, allow headings and actions to wrap, and keep floating panels within the viewport. Tables may scroll horizontally when preserving column relationships is necessary. Maintain useful control sizes and accessible navigation at narrow widths.

## Components and when to use them

Use shared components for common tasks. Extend their shared presentation and behavior when a reusable need emerges, so equivalent controls continue to look and work alike.

### Actions

| Treatment | Use | Examples |
| --- | --- | --- |
| Primary | The main action within a page or task group | Save changes, Create resource, Export |
| Secondary | Supporting actions | Refresh, Preview, Duplicate |
| Ghost | Quiet dismissal or low-emphasis actions | Cancel, Close |
| Danger | Confirmation of a destructive operation | Delete resource |

Make the main action visually distinct without giving every button equal emphasis. An action menu groups closely related choices, such as export formats. Its trigger follows the same visual hierarchy as other buttons.

Use concrete action labels. Copy controls show confirmation at the control itself. Loading and success labels should not cause avoidable width changes or move nearby controls. Use recognizable icons with consistent stroke weight and optical size; icon-only buttons need an accessible name.

### Fields and selection

Give every field a visible label. Use helper text for requirements, examples and consequences; placeholders supplement labels rather than replacing them. Clearly identify optional fields, defaults and inherited values.

| Control | Use |
| --- | --- |
| Text input | A short value such as a name, address or subject |
| Text area | Multiline plain text |
| Single select | One choice from a defined set |
| Checkbox | An independent boolean choice |
| Searchable multi-select | Several choices from a larger set |
| Secret field | Sensitive values with appropriate masking and reveal controls |

Selection controls need a clear selected state, predictable keyboard behavior and readable option labels. Keep selections intact while searching or loading additional options. Choose native or custom presentation according to the interaction need while preserving accessibility and the shared visual language.

Distinguish read-only values from disabled controls. Explain important restrictions near the affected action. Creation and editing must use appropriate guidance: references to a saved value belong only where a saved value exists.

### Lists, tables and empty states

Use consistent search, headings, row actions and pagination across management screens. Align numeric data for comparison and give actions their own clear location. The clickable area should match the visible control.

Distinguish an empty collection from a search with no matches, a pending load and a failed request. Empty states explain a useful next action. Avoid showing a table header without meaningful content beneath it. Pagination controls reflect whether another page is available.

### Tabs, disclosures and dialogs

Use tabs for closely related views of the same task, such as editing and previewing content. Switching views preserves the draft and relevant state. Make the active tab obvious and support keyboard navigation.

Use disclosures for supporting details that would otherwise clutter the page. Opening one should preserve the context of the item being inspected. Its content uses the width appropriate to that information.

Use dialogs for focused decisions or short tasks that belong to the current context. Give each dialog a clear title, a visible way to dismiss it and an explicit primary action. Destructive confirmations identify the affected object and consequence. Modal dialogs contain focus and return it to an appropriate control when closed.

Explanations must work with keyboard and touch input as well as hover. Use a labeled information control when help needs to be available without occupying the main layout.

## State, feedback and accessibility

Design the normal, focused, selected, disabled, loading, empty, successful and failed states together. Keep their presentation consistent across features.

Show progress where work is happening. Distinguish a queued or running operation from its completed result. Prevent duplicate actions where necessary, and restore controls when work finishes or fails.

Errors identify what failed and how to recover. Place field errors near their fields and use a broader message for page-level failures. Preserve the user's entered values. Announce significant errors and results accessibly without relying on color alone.

Keep focus visible, labels associated with controls and reading order consistent with visual order. Floating panels support predictable dismissal. Respect reduced-motion preferences; animation should clarify a response or ongoing activity.

## Charts and data presentation

Charts answer a clear question and use the available width. Label axes and units, choose meaningful tick intervals and adapt time labels to the visible duration. Counts use integer ticks. Avoid labels that overlap or imply precision the data does not support.

| Observation | Color token | Color |
| --- | --- | --- |
| Accepted | `--chart-accepted` | `#28629b` |
| Opened | `--chart-opened` | `#007d77` |
| Clicked | `--chart-clicked` | `#8962ad` |
| Submitted | `--chart-submitted` | `#aa6420` |
| Reported | `--chart-reported` | `#465465` |

Keep each category's color consistent across its chart, legend and detail views. Pair colors with labels and visible marks. Exact values and their time intervals should be available through keyboard and pointer interaction.

Distinguish totals, unique recipients, missing data and zero values. Explain observation limits in plain language. Describe measured outcomes accurately: an image fetch is an observed open, and SMTP acceptance does not establish inbox delivery.

## Writing and extending the system

Use sentence case, familiar language and specific verbs. Keep action and result wording consistent: "Save changes" leads to a clear saved confirmation. Avoid technical implementation details in operator-facing copy unless they help the operator make a decision.

When adding a screen or component:

1. Identify the operator's task, main action and important states.
2. Reuse the closest shared layout and interaction patterns.
3. Apply semantic color, typography and spacing tokens.
4. Check alignment, wrapping, keyboard access and state feedback.
5. Extend the shared component when the need applies across features.

Keep this guide focused on reusable design rules and usage guidance. Component API documentation, build instructions, implementation constraints and historical decisions belong in their respective technical documentation.
