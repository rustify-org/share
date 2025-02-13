import {defineConfig} from 'tsup'

export default defineConfig({
  entry: ["src/page1.ts", "src/page2.ts"],
  clean: true,
  format: ["esm"],
  sourcemap: false
})