import test from 'ava'

import { sum, sub } from '../index.js'

test('sum from native', (t) => {
  t.is(sum(1, 2), 3)
})

test('sub from native', (t) => {
  t.is(sub(1, 2), -1)
})
