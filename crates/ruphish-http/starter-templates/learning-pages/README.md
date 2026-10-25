# Learning starter content

These are three independent, equally selectable English learning pages. Each HTML document is authored separately, with its own layout and CSS. All use the shared catalog logo-replacement capability through a single `.learning-logo` container; none uses recipient prefill or executable content.

| Directory | Method reference | Composition |
| --- | --- | --- |
| `slam/` | [University of Louisville: SLAM](https://louisville.edu/its/cybersecurity/phishing-and-scams) | Four-letter message inspection sheet with local examples. |
| `stop-challenge-protect/` | [Take Five: Stop, Challenge, Protect](https://www.takefive-stopfraud.org.uk/) | Three sequential full-width stages with verification questions. |
| `recognize-resist-report-delete/` | [CISA phishing tip sheet](https://www.cisa.gov/sites/default/files/2024-09/Secure-Our-World-Phishing-Tip-Sheet.pdf) | Sidebar overview with response steps and reporting before deletion. |

Explanations and examples are original Ruphish wording, with method attribution in each page. The latter two methods are adapted for workplace reporting and recovery. External references are ordinary links; loading a page does not fetch them. Examples are illustrative and never describe a recipient’s actual campaign message or actions. Reporting instructions intentionally refer to the organisation’s established process so an operator can tailor an imported copy without shipping an invented address or helpdesk URL.

The embedded logo comes from `crates/ruphish-ui/web/branding/ruphish-wordmark.svg` in the repository root. Its dark teal artwork is rasterized to a transparent PNG, 788 × 256 pixels, using `@resvg/resvg-js` with `fitTo: { mode: 'width', value: 788 }`, then encoded as a `data:image/png;base64,` image in each document. Rasterization is an authoring step, not a runtime dependency. The canonical SVG remains the artwork source; do not redraw the logo or link to the admin asset path. Its outlined lettering derives from IBM Plex Sans Bold, Copyright 2017 IBM Corp., under SIL OFL 1.1; the existing attribution is retained in each HTML source. No font is embedded in these pages.

The pages use system font stacks, local CSS, semantic headings, ordered steps and responsive layouts. Uploaded logos replace only the image slot. Keep the white logo backing to support dark artwork and preserve responsive sizing for both wide and tall uploads. Content and branding transformations are covered by the starter functional tests and executable public-serving workflow described in [the contributor guide](../../../../docs/contributing/starter-templates.md).
