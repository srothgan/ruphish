---
title: Run your first local campaign
description: Create a campaign with synthetic recipients and inspect its message and results in the local fixture.
sidebar:
  order: 5
---

This exercise uses Ruphish's existing local fixture: an isolated application database, test DNS and HTTPS, and a Mailpit inbox. All recipients use `.test` addresses. Messages stay in the supplied capture service when you keep the sending profile below unchanged.

You'll create your own resources through the interface, launch a small campaign, and inspect its results. The fixture's `seed` command is not needed for this walkthrough; it creates a separate demonstration campaign automatically.

## Prepare the fixture

Complete the [build prerequisites and browser build](installation.md#build). You also need Docker with Linux containers and Compose v2. Run commands from `ruphish/`:

```sh
node tests/acceptance/lab.mjs init
docker compose -f tests/acceptance/compose.yaml --profile setup run --rm prepare
docker compose -f tests/acceptance/compose.yaml build app
docker compose -f tests/acceptance/compose.yaml up -d --wait dns inbox app
node tests/acceptance/lab.mjs demo
```

Preparation builds the Linux executable and test certificates; the image build packages the executable and completed admin UI. The first run needs dependency downloads. Initialization preserves existing fixture data and credentials. `demo` completes the fixture account's initial password change, enables domain verification for the demonstration, publishes the local domain proof, and verifies the passing origin. It also keeps a deliberately failing domain for comparison.

Open `http://127.0.0.1:3334`. Retrieve the fixture password locally:

```sh
node tests/acceptance/lab.mjs password
```

Sign in as `admin`. Under **Domains**, `https://good.ruphish.test:8443` should be eligible for campaign use. Leave the wrong-TXT example alone.

## Open the exercise browser

The public `.test` hostname needs a local mapping and trust for the fixture certificate. Use an isolated Chromium profile rather than changing system DNS or trust. On Windows with Chrome installed, run in PowerShell:

```powershell
$pin = node tests/acceptance/lab.mjs browser-pin
$profile = Join-Path (Get-Location) 'tests/acceptance/data/browser-profile'
& 'C:\Program Files\Google\Chrome\Application\chrome.exe' "--user-data-dir=$profile" '--host-resolver-rules=MAP good.ruphish.test 127.0.0.1, MAP bad.ruphish.test 127.0.0.1' "--ignore-certificate-errors-spki-list=$pin" 'http://127.0.0.1:8026'
```

Adjust the executable path if needed. Linux Chromium instructions and fixture troubleshooting are in [the local fixture guide](../contributing/testing/local-fixture.md). Close this dedicated browser completely before changing its flags. The certificate exception is scoped to the fixture key; do not substitute a general certificate-validation bypass.

## Create a sending profile

Open **Resources → Sending profiles** and choose **Create**:

| Field | Value |
| --- | --- |
| Name | `First exercise inbox` |
| SMTP host | `172.30.53.20:1025` |
| From address | `Training <training@example.test>` |
| Username/password | Leave empty |

Save, open **Send test**, and choose **Test connection only**. This address is the fixture inbox on its private Docker network. It is not the address to use for an unrelated native installation. A successful connection test confirms connectivity; it has not sent a campaign.

## Import two recipients

Create a UTF-8 file named `first-exercise.csv` with this content:

```csv
First Name,Last Name,Email,Position
Alex,Rivera,alex@example.test,Operations
Sam,Lee,sam@example.test,Support
```

Open **Groups → Import**, select the file, and inspect the column mapping and accepted rows. Continue to the create form, name the group `First exercise audience`, and save. Confirm both addresses appear. If you repeated the tutorial, choose a new resource name rather than overwriting previous work.

## Write the email and landing page

Create an email template named `First exercise email`, with subject `A local exercise for {{.FirstName}}`. Put this in its HTML source:

```html
<p>Hello {{.FirstName}},</p>
<p><a href="{{.URL}}">Open the local training page</a>.</p>
{{.Tracker}}
```

Add the plain-text alternative:

```text
Hello {{.FirstName}},
Open the local training page: {{.URL}}
```

Save the template. Create a landing page named `First exercise page` with:

```html
<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>Local training exercise</title></head>
<body>
  <h1>You've reached the training page</h1>
  <p>This is a local example. No sign-in is needed.</p>
</body>
</html>
```

Leave submission tracking and the redirect unset for this first exercise. Preview and save. A preview does not create campaign events.

## Review and launch

Open **Campaigns → Create campaign**. Name it `My first local campaign`, select your group, email, page, and sending profile, and choose the verified passing domain. Leave the schedule empty for immediate delivery.

Review the audience and personalized message. You should see two unique recipients and the fixture's HTTPS origin. If the domain is unavailable, return to **Domains** and inspect its latest check before proceeding.

Choose **Launch campaign**. Open Mailpit at `http://127.0.0.1:8026` and find the messages by subject and recipient. The greetings and campaign links should be personalized. SMTP acceptance means the capture service accepted the mail; in a real installation, it would not establish inbox placement.

## Inspect the result

In the dedicated exercise browser, follow Alex's campaign link from Mailpit. The training page should open. Return to the campaign and choose **View analysis**. Inspect Alex's timeline and the click observation. An image open appears only if the mail viewer requests the tracking image; the click can count toward summary progression even without a separate open event.

Download **Recipients CSV** or **Events CSV**. The recipient export summarizes people; the event export lists observations. Use [metric definitions](../user-guide/results/metrics.md) to understand the difference.

## Finish

Complete the campaign when you're done. New public interactions no longer update its history. Completion retains the campaign for review; deletion removes it.

Stop the local services without deleting their data:

```sh
docker compose -f tests/acceptance/compose.yaml stop app inbox dns
```

Close the dedicated browser. Delete only your tutorial resources through the interface if you want to remove them; referenced resources may require deleting the campaign first. Do not remove the fixture data directory to clean up one exercise.

Next, explore [scheduled campaigns](../user-guide/campaigns/scheduling.md), [landing-page forms](../user-guide/landing-pages.md), or [the API](../api/index.md).
