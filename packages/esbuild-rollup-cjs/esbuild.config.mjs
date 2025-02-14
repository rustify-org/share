import { build } from "esbuild";

build({
  entryPoints: ["src/esbuild-entry.js"],
  bundle: true,
  format: "esm",
  outfile: "dist/esbuild-bundle.js",
})
  .then(() => console.log("Esbuild build complete!"))
  .catch(() => process.exit(1));
