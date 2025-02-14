import commonjs from "@rollup/plugin-commonjs";
import { nodeResolve } from "@rollup/plugin-node-resolve";

export default {
  input: "src/rollup-entry.js",
  output: {
    file: "dist/rollup-bundle.js",
    format: "esm",
  },
  plugins: [commonjs(), nodeResolve()],
};
