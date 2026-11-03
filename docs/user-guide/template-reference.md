---
title: Personalize content
description: Use recipient fields, conditional text, and escaping in campaign content.
sidebar:
  order: 41
---

Personalization inserts recipient and campaign details into email subjects, message bodies, landing pages, supported attachments, and campaign URL paths or queries. Learning pages are static and do not evaluate these expressions.

Expressions use Go-style template syntax inside `{{` and `}}`. Names are case-sensitive.

## Choose a field

| Expression | Value |
| --- | --- |
| `{{.FirstName}}` | Recipient's first name. |
| `{{.LastName}}` | Recipient's last name. |
| `{{.Email}}` | Recipient's email address. |
| `{{.Position}}` | Recipient's position. |
| `{{.From}}` | The visible sender context for the message. |
| `{{.URL}}` | This recipient's campaign link. |
| `{{.BaseURL}}` | The campaign's base address, without its path or query. |
| `{{.RId}}` | The recipient's public tracking identifier. |
| `{{.TrackingURL}}` | The tracking-image address. |
| `{{.Tracker}}` | Tracking-image HTML. |

Use `{{.URL}}` for campaign links. You do not need to insert a tracking image yourself: **Track email opens** controls whether Ruphish adds one to HTML email.

A known field may be empty when the recipient has no value. An unknown field, such as `{{.email}}`, fails rendering instead of producing an empty value.

## Handle missing names

Use a condition when a field is optional:

```text
Hello {{if .FirstName}}{{.FirstName}}{{else}}there{{end}},
Your campaign link: {{.URL}}
```

For HTML, escape inserted values:

```html
<p>Hello {{if .FirstName}}{{.FirstName | html}}{{else}}there{{end}},</p>
<p><a href="{{.URL | html}}">Open the document</a></p>
```

This lets a name such as `Alex & Sam` appear as text rather than becoming part of the markup.

## Escape for the destination

Template evaluation does not automatically escape HTML. Use `html` for inserted HTML text and quoted attributes, and `urlquery` for a value inside a URL query.

For example, a campaign path could include `/document?recipient={{.Email | urlquery}}`. Keep the scheme, hostname, and port fixed; they must match the selected domain.

Avoid inserting recipient values into executable JavaScript. The page sandbox does not replace correct escaping.

## Use expressions

The renderer supports conditions with `if`/`else`, `with`, `range`, variables, named templates, comments, and pipelines.

Common functions include `and`, `or`, `not`, comparisons such as `eq` and `ne`, formatting with `printf`, and `len`, `index`, `slice`, `html`, `js`, and `urlquery`. Functions require appropriate argument types; recipient fields are strings, not arbitrary collections or objects.

```text
{{if eq .Position "Operations"}}Operations update{{else}}Team update{{end}}
{{printf "%s %s" .FirstName .LastName}}
```

String length and slicing follow byte-based behavior. A slice that splits a UTF-8 character fails, so avoid byte slicing when you only need to display a person's name.

## Check personalized output

Preview with representative recipients, including missing optional values and names containing accents or punctuation. Send a test message and open any personalized attachments.

If rendering fails, check expression spelling, matching `end` statements, and function arguments. Rendering has limits on document size and expression work; simplify unusually large or deeply nested templates. Ruphish does not send a partial rendering as a successful message.

See [Email templates](resources/email-templates.md#add-attachments) for supported attachment formats.
