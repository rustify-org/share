import test from 'ava'

import { sum, sub, concatStr, getOptions } from '../index.js'

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