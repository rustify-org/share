// 带缓存的 asyncFib 方法
const cache = new Map();

export const asyncFib = async (n, useCache) => {
  if (useCache && cache.has(n)) {
    return cache.get(n);
  }
  
  if (n <= 1) {
    return n;
  }
  
  const result = await asyncFib(n - 1, useCache) + await asyncFib(n - 2, useCache);
  
  if (useCache) {
    cache.set(n, result);
  }
  
  return result;
};

// 不带缓存的 asyncFib 方法
export const asyncFibWithoutCache = async (n) => {
  if (n <= 1) {
    return n;
  }  
  return await asyncFibWithoutCache(n - 1) + await asyncFibWithoutCache(n - 2);
};
