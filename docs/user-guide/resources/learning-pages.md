---
title: Learning pages
description: Prepare educational content to show after a campaign submission.
sidebar:
  order: 15
---

A learning page explains the exercise and helps recipients recognize similar messages in the future. A campaign can show it after a confirmed form submission.

Learning pages contain static educational content. Visits do not record a new campaign interaction or prove that someone completed training.

## Create a learning page

1. Open **Resources → Learning pages** and choose **Create**.
2. Enter a name that explains the lesson or audience.
3. Write the page in the HTML or visual editor, or import HTML or resource JSON.
4. Choose **Check learning page**, then inspect **Preview**.
5. Apply any visual-editor changes and choose **Create**.

You can also add a ready-made lesson through [Starter templates](../starter-templates.md). Customize its reporting instructions and contact details for your organization.

Keep the explanation useful to someone who has just interacted with the message: show the clues they could check, explain what to do next, and tell them how your organization expects suspicious messages to be reported.

## Understand the content limits

Learning pages support static text, styling, permitted embedded images, and links. Forms and input controls are removed from the public page. Scripts, external resource loading, and automatic redirects are blocked.

Recipient personalization is not applied. An expression such as `{{.FirstName}}` appears literally, so write the lesson to work for everyone.

Your original HTML remains editable. Preview shows the restricted version recipients will see; use it to check the page instead of relying only on the source.

## Use the page in a campaign

1. Create or edit a campaign draft.
2. Under **Redirect destination**, choose **Learning page**.
3. Select the saved page.
4. Review the destination and launch or schedule the campaign.

Ruphish creates the destination address on the campaign's domain. You do not need to copy a URL into the campaign.

## Update or remove learning content

Editing the resource updates its standalone public page and the content available to future campaigns. A launched campaign keeps a separate saved copy of its learning content.

The page's public link requires no Ruphish login. It is available through an enabled, eligible campaign domain. A campaign's saved learning page can remain available after campaign completion; deleting the campaign removes that saved link.

Delete a learning-page resource only when you no longer need its standalone page or future reuse. Ruphish blocks deletion while an unfinished campaign references it.
