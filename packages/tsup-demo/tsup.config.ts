import {defineConfig} from 'tsup'

export default defineConfig({
  entry: ["src/page1.ts", "src/page2.ts", "src/page3.ts", "src/page4.ts"],
  clean: true,
  format: ["esm"],
  sourcemap: false
})

// ! esbuild大部分应用在transformer 而不是bundle 大量的小chunk IO的开销巨大 