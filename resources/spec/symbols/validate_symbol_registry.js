#!/usr/bin/env node

const fs = require('node:fs');
const path = require('node:path');

const REQUIRED_CATEGORY_KEYS = [
  'ca_delimiter_symbols',
  'ca_element_symbols',
  'word_segment_forbidden_start_symbols',
  'word_segment_forbidden_rest_symbols',
  'word_segment_forbidden_common_symbols',
  'event_segment_forbidden_symbols',
  'event_segment_forbidden_common_symbols',
];

function fail(message) {
  console.error(`symbol registry validation failed: ${message}`);
  process.exit(1);
}

function isSingleUnicodeScalar(value) {
  return typeof value === 'string' && value.length > 0 && [...value].length === 1;
}

function ensureSortedUnique(name, values) {
  if (!Array.isArray(values) || values.length === 0) {
    fail(`${name} must be a non-empty array`);
  }

  const seen = new Set();
  for (const symbol of values) {
    if (!isSingleUnicodeScalar(symbol)) {
      fail(`${name} entries must be single Unicode scalar values, got: ${JSON.stringify(symbol)}`);
    }
    if (seen.has(symbol)) {
      fail(`${name} contains duplicate symbol: ${JSON.stringify(symbol)}`);
    }
    seen.add(symbol);
  }

  // No sort requirement — semantic grouping is more useful than forced ordering.
}

function ensureDisjoint(nameA, arrA, nameB, arrB) {
  const setA = new Set(arrA);
  const overlap = arrB.filter((value) => setA.has(value));
  if (overlap.length > 0) {
    fail(`${nameA} and ${nameB} must be disjoint, overlap: ${overlap.map((s) => JSON.stringify(s)).join(', ')}`);
  }
}

function main() {
  const repoRoot = path.resolve(__dirname, '..', '..');
  const registryPath = path.join(repoRoot, 'spec', 'symbols', 'symbol_registry.json');

  if (!fs.existsSync(registryPath)) {
    fail(`missing registry file at ${registryPath}`);
  }

  let registry;
  try {
    registry = JSON.parse(fs.readFileSync(registryPath, 'utf8'));
  } catch (err) {
    fail(`invalid JSON: ${err.message}`);
  }

  if (!Number.isInteger(registry.version) || registry.version <= 0) {
    fail('version must be a positive integer');
  }
  if (typeof registry.description !== 'string' || registry.description.trim().length === 0) {
    fail('description must be a non-empty string');
  }
  if (!registry.categories || typeof registry.categories !== 'object') {
    fail('categories must be an object');
  }

  for (const key of REQUIRED_CATEGORY_KEYS) {
    if (!(key in registry.categories)) {
      fail(`missing required category: ${key}`);
    }
    ensureSortedUnique(key, registry.categories[key]);
  }

  ensureDisjoint(
    'ca_delimiter_symbols',
    registry.categories.ca_delimiter_symbols,
    'ca_element_symbols',
    registry.categories.ca_element_symbols,
  );

  console.log('symbol registry validation: ok');
  for (const key of REQUIRED_CATEGORY_KEYS) {
    console.log(`  - ${key}: ${registry.categories[key].length}`);
  }
}

main();
