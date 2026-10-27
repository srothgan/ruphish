# Public demo

Static build of the real administration UI. Sign in with **demo / demodemo**. Changes and external actions are simulated; the sample data stays unchanged.

From the repository root, with the [pinned development tools](../docs/contributing/setup.md) installed:

```sh
npm ci --prefix crates/ruphish-ui/web
node crates/ruphish-ui/web/build-demo.mjs
trunk build --locked --features demo
node demo/serve.mjs
```

Open `http://127.0.0.1:8080`. Stop with Ctrl+C. The completed artifact is `dist/admin`; rebuild with `trunk build --locked` before using that directory for the normal application.

See the [demo development guide](../docs/contributing/demo.md) for architecture, dataset ownership and GitHub-to-Vercel deployment setup, and [validation](../docs/contributing/testing/validation.md#public-demo) for test commands. Local preview uses the development build; CI packages an optimized artifact for deployment. Visual review remains a user task.
