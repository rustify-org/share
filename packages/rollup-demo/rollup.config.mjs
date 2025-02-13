import { defineConfig } from 'rollup';

export default defineConfig({
    input: 'src/index.js',
    output: {
      file: 'dist/bundle.js',
      format: 'iife', // iife、cjs、umd 等格式支持 strict
      // strict: false, // 关闭 strict mode
    },
 })
