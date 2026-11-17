// Time formats and typed times (dates.js):  node --test ui/dates.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { friendlyTime, parseTime, useClock } from './dates.js';

test('times are 24-hour unless the user chose AM/PM', () => {
  useClock({ hour12: false });
  assert.deepEqual(['00:05', '09:00', '14:30', '23:59'].map(friendlyTime), ['0:05', '9:00', '14:30', '23:59']);
  useClock({ hour12: true });
  assert.deepEqual(['00:05', '09:00', '12:00', '14:30'].map(friendlyTime), ['12:05 AM', '9:00 AM', '12:00 PM', '2:30 PM']);
  useClock({ hour12: false });
});

test('typed times', () => {
  const cases = {
    '14:30': '14:30', '1430': '14:30', '930': '09:30', '9': '09:00', '9.15': '09:15', '14h30': '14:30', ' 7:05 ': '07:05',
    '2pm': '14:00', '2:30 PM': '14:30', '12am': '00:00', '12:15 pm': '12:15', '11 a': '11:00',
  };
  for (const [typed, want] of Object.entries(cases)) assert.equal(parseTime(typed), want, typed);
  for (const bad of ['', '24:00', '9:60', '13pm', 'noon', '1:2', '12345']) assert.equal(parseTime(bad), null, bad);
});
