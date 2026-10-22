# Security policy

Report suspected security vulnerabilities through GitHub private vulnerability reporting. Do not post exploit details in public issues, discussions, or pull requests.

> TODO before publication: Once `srothgan/ruphish` is public, enable GitHub private vulnerability reporting, verify that external reporters can use it, and add the direct reporting link here. GitHub does not offer this reporting channel for private repositories.

## Report a vulnerability

In the repository, open **Security → Advisories → Report a vulnerability** to start a private report. If that option is unavailable, do not submit vulnerability details through a public issue. You may open a blank issue asking only for a private reporting contact, without describing the vulnerability or affected installation.

Include what you know; a complete exploit or proposed fix is not required:

- The affected Ruphish version or source commit and relevant installation details, such as database provider, browser, or proxy configuration.
- What an attacker can do, which permissions or access they need, and the expected security boundary.
- Steps to reproduce using synthetic data, with a minimal proof of concept or redacted evidence if available.
- Any workaround or suggested fix, if known.

Do not attach live databases, API keys, session cookies, private keys, service credentials, recipient lists, or participant-entered data. Test only against installations you own or are authorized to assess.

## Scope

Reports may concern the Ruphish server, administration interface, HTTP APIs, migrations, or distributed assets. Relevant boundaries include account and resource ownership, authentication, participant-data handling, outbound connections, and the handling of authored email or landing-page content.

If a dependency issue affects Ruphish, include the dependency or advisory and explain the affected Ruphish behavior. A dependency advisory alone may not establish that the vulnerable behavior is reachable.

## Coordinate disclosure

Keep reproduction details in the private report while discussing impact, remediation, and publication with the maintainers. State whether and how you would like to be credited. Response and resolution times depend on the issue; this policy does not promise a fixed deadline.
