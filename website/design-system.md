# Ruphish website design system

Adapted from the Ruphish admin design guide before its archival directory is removed. The website carries the same visual identity into product explanation and long-form documentation. The application and website have independent builds; this guide owns the website's presentation.

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
| Improving outcome | `--success` | `#187344` | Favorable metric changes; pair with direction and text |
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

## Website composition

The landing page introduces Ruphish with one short headline, a clear explanation, and Get started and View source actions. A real dashboard screenshot is the central visual. Keep its proportions, show synthetic data, label the image as an example, and let readers open it at full resolution. Do not recreate application numbers or controls in website markup.

Use a centered hero and screenshot. Align feature prose and the About page to the left. The headline scales from approximately 42px on narrow screens to 84px on wide screens; body text is 17?19px. Use larger 48?100px spacing between website sections while retaining the original 4px spacing scale within components. Primary website actions are at least 48px tall.

Keep the landing page light, with pale teal canvas and white image surfaces. Starlight retains its light/dark theme control; adapt both themes through semantic color tokens. Self-host fonts. Use the original SVG wordmark in navigation, its white variant on the footer, and the R symbol as the favicon. Preserve attribution for the IBM Plex lettering.

## Documentation experience

Starlight owns navigation, search, theme selection, headings, code blocks, on-page contents, and previous/next links. Customize tokens and carefully scoped components instead of rebuilding these features. Organize the sidebar by reader goal: getting started, user guide, administration, HTTP API, migration, and contributing. Keep the getting-started section expanded and supporting sections collapsible.

Keep documentation prose around 49rem wide, with comfortable line height, clearly distinguished code, and horizontally scrollable wide tables. Search must retain its keyboard shortcut and mobile dialog. Edit links point to the authoritative Markdown under the repository's docs directory. The shared footer includes project links, licensing, and a Gophish acknowledgment while preserving Starlight's page navigation.

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
