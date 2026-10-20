// Build-only asset generation. Verification commands remain separate in AGENTS.md.
import { build } from "esbuild";
import { createHash } from "node:crypto";
import { cp, mkdir, readdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const web = path.dirname(fileURLToPath(import.meta.url));
if (process.argv[2] === "manifest") {
  const root = process.env.TRUNK_STAGING_DIR;
  if (!root) throw new Error("TRUNK_STAGING_DIR is required for manifest generation");
  const files = {};
  async function collect(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const absolute = path.join(directory, entry.name);
      if (entry.isDirectory()) await collect(absolute);
      else if (entry.isFile()) {
        const name = path.relative(root, absolute).split(path.sep).join("/");
        files[name] = createHash("sha256").update(await readFile(absolute)).digest("hex");
      } else throw new Error(`Unsupported build output: ${absolute}`);
    }
  }
  await collect(root);
  await writeFile(path.join(root, "asset-manifest.json"), `${JSON.stringify(files, null, 2)}\n`);
  // Trunk stages under dist/.stage and promotes its contents into dist/.
  // Package completed assets as one admin subtree so the server's dist/admin
  // directory never contains Trunk's temporary build files.
  const entries = await readdir(root, { withFileTypes: true });
  const admin = path.join(root, "admin");
  await mkdir(admin); // A conflicting generated admin entry is a build error.
  for (const entry of entries) {
    const source = path.resolve(root, entry.name);
    const destination = path.join(admin, entry.name);
    if (path.dirname(source) !== path.resolve(root)) throw new Error("Asset path escapes staging directory");
    if (entry.isDirectory()) {
      // On Windows, moving previously enumerated subdirectories can prevent
      // Trunk from renaming their new parent. Materialize fresh directories;
      // only remove the original staged subtree after its copy completes.
      await cp(source, destination, { recursive: true, errorOnExist: true, force: false });
      await rm(source, { recursive: true });
    } else await rename(source, destination);
  }
} else if (process.argv.length === 2) {
  await mkdir(path.join(web, "generated"), { recursive: true });
  const common = { bundle: true, minify: true, target: "es2022", legalComments: "linked", logLevel: "info" };
  await Promise.all([
    build({ ...common, entryPoints: [path.join(web, "../ruphish.css")], external: ["fonts/*"], outfile: path.join(web, "generated/admin.css") }),
    build({ ...common, entryPoints: [path.join(web, "editor.mjs")], format: "esm", outfile: path.join(web, "generated/editor.js") }),
    build({ ...common, entryPoints: [path.join(web, "visual-frame.mjs")], format: "iife", loader: { ".svg": "text", ".less": "empty" }, outfile: path.join(web, "generated/visual-frame.js") }),
  ]);
} else throw new Error("Expected no argument or manifest");
