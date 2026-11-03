---
title: Contributor documentation
description: Build a development checkout, work on the code or content, and verify your changes.
sidebar:
  order: 0
prev: false
next:
  link: /docs/contributing/development-setup/
  label: Build and run locally
---

Start with [CONTRIBUTING.md](../../CONTRIBUTING.md) for proposing work, submitting pull requests, and working through review. These guides explain how to do the technical work.

| Your task | Start here |
| --- | --- |
| Build and run Ruphish from source | [Development setup](development-setup/index.md) |
| Find where a change belongs | [Architecture](development-guides/architecture.md) |
| Change server behavior or persistence | [Backend development](development-guides/backend.md), [database development](development-guides/database-development.md) |
| Change the application interface | [Frontend development](development-guides/frontend.md), [design system](development-guides/design-system.md) |
| Translate the interface | [Localization](development-guides/localization.md) |
| Improve the docs or website | [Documentation and website](development-guides/documentation.md) |
| Add bundled content or change the demo | [Starter templates](development-guides/starter-templates.md), [public demo](development-guides/demo.md) |
| Choose and run checks | [Testing](testing/index.md) |
| Maintain CI, releases, or demo deployment | [Maintenance](maintenance/index.md) |

For your first code change, set up the checkout, use the architecture guide to locate the implementation, and select checks for the behavior you change. Documentation-only work needs the website tools, not a Rust build.
