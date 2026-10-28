---
title: Template reference
description: Personalize email, pages, URLs, and supported attachments with Go-template expressions.
sidebar:
  order: 5
---

Ruphish evaluates Go-style text templates. Expressions are case-sensitive and enclosed in `{{` and `}}`. They are executed expressions, not a simple string-replacement vocabulary.

## Available fields

| Expression | Value |
| --- | --- |
| `{{.FirstName}}` | Recipient's first name |
| `{{.LastName}}` | Recipient's last name |
| `{{.Email}}` | Recipient's email address |
| `{{.Position}}` | Recipient's position |
| `{{.From}}` | Sender context supplied for the message |
| `{{.URL}}` | Personalized campaign URL |
| `{{.RId}}` | Opaque public recipient identifier |
| `{{.BaseURL}}` | Campaign origin without its path/query |
| `{{.TrackingURL}}` | Tracking-image URL |
| `{{.Tracker}}` | Complete tracking-image markup |

These tracking fields remain available for legacy templates. New email templates need neither: the sender automatically inserts one hidden tracking image in HTML, and previews remove rendered tracking images. Plain-text-only emails remain plain text.

A recognized field can be empty. An unknown field, including a wrong-case field such as `.email`, produces an execution error rather than silently becoming an empty string.

## Conditional content and formatting

```text
Hello {{if .FirstName}}{{.FirstName}}{{else}}there{{end}},
Your exercise link: {{.URL}}
```

Variables, `if`/`else`, `with`, `range`, named templates with `define`/`template`, comments, and pipelines are supported by the renderer. Expressions must receive appropriate values: for example, ranging over a recipient's name is not a supported collection operation.

```text
{{$name := .FirstName}}{{with .LastName}}{{$name}} {{.}}{{end}}
{{printf "%s" .Email | printf "[%s]"}}
```

The renderer includes Go-style built-ins for boolean/comparison operations, formatting, indexing/slicing, length, and explicit escaping. Use the ordinary text-template functions (`and`, `or`, `not`, `eq`, `ne`, `lt`, `le`, `gt`, `ge`, `print`, `printf`, `println`, `index`, `slice`, `len`, `html`, `js`, and `urlquery`) with arguments of the appropriate type. Application-specific Sprig functions or arbitrary Go methods are not an extension interface.

## Escaping and Unicode

Text-template evaluation does not automatically HTML-escape recipient values. Choose explicit escaping for the insertion context, such as `{{html .FirstName}}` for HTML text. Avoid inserting recipient values into executable code. Authored landing pages permit scripts after expansion, so unescaped recipient markup can become executable too. The [participant-page sandbox](participant-data.md) isolates that document from administration; it does not provide contextual escaping or prevent code from reading the page's controls.

String length and indexing follow byte-oriented compatibility behavior. A slice that splits a UTF-8 character is rejected, rather than returning invalid UTF-8. For example, `{{slice "José" 3 5}}` yields `é`; slicing only one byte of that character fails.

## Errors and limits

Invalid syntax fails parsing; missing fields, invalid function arguments, or an absent named template can fail execution. Rendering is bounded for nesting, iteration, allocation, and output. Resource documents and rendered output have an 8 MiB limit. Treat a failed render as an error, not as a usable partial message.

Preview with representative values before launch, including empty optional names and non-ASCII text. Template rendering does not fetch arbitrary external data. Imported content retains its expressions; import is not a conversion to a different template language.

See [email templates](email-templates.md), [landing pages](landing-pages.md), and [attachments](attachments.md) for where these expressions are used.
