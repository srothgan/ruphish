---
title: Maintain starter templates
description: Update the bundled catalog and its supported customization options.
---

The bundled catalog lives in `crates/ruphish-http/starter-templates/`. Each email includes HTML, a plain-text alternative and a subject. Keep asset attribution and license notices with the content.

When adding or updating an entry:

1. Review the email or landing page individually, including its preview and form behavior.
2. Update its catalog metadata in `crates/ruphish-http/src/starter_templates.rs`.
3. Declare prefill and company-logo support only where the template supports them. Keep the [company-logo availability list](../user-guide/starter-templates.md#company-logo-availability) current.
4. Keep all required assets offline and preserve supported Go-template expressions.
5. Extend the relevant catalog and transformation tests, following [validation](testing/validation.md).

Emails and landing pages are independent entries. Imported copies are ordinary resources with no catalog relationship, installed status or automatic updates.
