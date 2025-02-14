import greet from "./cjs-func.js";

console.log(greet("Esbuild"));  // 期望调用函数
console.log(greet.extra);       // 访问额外属性
