import test from 'ava'

import { sum, sub, concatStr, getOptions, asyncFib } from '../index.js'

test('sum from native', (t) => {
  t.is(sum(1, 2), 3)
})

test('sub from native', (t) => {
  t.is(sub(1, 2), -1)
})

test('concat str from native', (t) => {
  t.is(concatStr('Hello', ' World'), 'Hello World')
})

test('get options from native', (t) => {
  t.deepEqual(getOptions({
    id: 1,
    name: 'Sunny'
  }), {
    id: 1,
    name: 'Sunny'
  })
})

test('asyncFib without cache', async (t) => {
  console.time('without cache');
  const result = await asyncFib(40, false);
  console.timeEnd('without cache');
  t.is(result, 102334155);
});

test('asyncFib with cache', async (t) => {
  console.time('with cache');
  const result = await asyncFib(40, true);
  console.timeEnd('with cache');
  t.is(result, 102334155);
});
