#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, '..');
const grammarRoot = path.resolve(repoRoot, 'grammar');

const registryPath = path.join(repoRoot, 'spec', 'symbols', 'symbol_registry.json');
const outputPath = path.join(grammarRoot, 'src', 'generated_symbol_sets.js');

function fail(message) {
  console.error(`symbol registry generation failed: ${message}`);
  process.exit(1);
}

function ensureSymbolArray(name, value) {
  if (!Array.isArray(value) || value.length === 0) {
    fail(`${name} must be a non-empty array`);
  }

  const seen = new Set();
  for (const symbol of value) {
    if (typeof symbol !== 'string' || symbol.length === 0) {
      fail(`${name} entries must be non-empty strings`);
    }
    if ([...symbol].length !== 1) {
      fail(`${name} entries must be single Unicode scalar values, got: ${JSON.stringify(symbol)}`);
    }
    if (seen.has(symbol)) {
      fail(`${name} contains duplicate symbol: ${JSON.stringify(symbol)}`);
    }
    seen.add(symbol);
  }
}

function escapeForCharClass(symbol) {
  const codePoint = symbol.codePointAt(0);

  if (symbol === '\\') return '\\\\';
  if (symbol === '[') return '\\[';
  if (symbol === ']') return '\\]';
  if (symbol === '^') return '\\^';
  if (symbol === '-') return '\\-';
  if (symbol === '\n') return '\\n';
  if (symbol === '\r') return '\\r';
  if (symbol === '\t') return '\\t';

  if (codePoint === 0x2028 || codePoint === 0x2029 || codePoint < 0x20 || codePoint === 0x7f) {
    return `\\u${codePoint.toString(16).padStart(4, '0')}`;
  }

  return symbol;
}

function buildCharClass(name, symbols) {
  ensureSymbolArray(name, symbols);
  return symbols.map(escapeForCharClass).join('');
}

function escapeTemplateLiteral(value) {
  return value.replaceAll('`', '\\`').replaceAll('${', '\\${');
}

const rawRegistry = fs.readFileSync(registryPath, 'utf8');
const registry = JSON.parse(rawRegistry);
const categories = registry?.categories;

if (!categories || typeof categories !== 'object') {
  fail('registry.categories must be an object');
}

const requiredKeys = [
  'ca_delimiter_symbols',
  'ca_element_symbols',
  'word_segment_forbidden_start_symbols',
  'word_segment_forbidden_rest_symbols',
  'word_segment_forbidden_common_symbols',
  'event_segment_forbidden_symbols',
  'event_segment_forbidden_common_symbols',
];

for (const key of requiredKeys) {
  if (!(key in categories)) {
    fail(`missing required category: ${key}`);
  }
}

const delimiterSymbols = categories.ca_delimiter_symbols;
const elementSymbols = categories.ca_element_symbols;

ensureSymbolArray('ca_delimiter_symbols', delimiterSymbols);
ensureSymbolArray('ca_element_symbols', elementSymbols);

const delimiter = delimiterSymbols.join('');
const element = elementSymbols.join('');
const all = [...delimiterSymbols, ...elementSymbols].join('');

const wordStartBase = buildCharClass(
  'word_segment_forbidden_start_symbols',
  categories.word_segment_forbidden_start_symbols,
);
const wordRestBase = buildCharClass(
  'word_segment_forbidden_rest_symbols',
  categories.word_segment_forbidden_rest_symbols,
);
const wordCommon = buildCharClass(
  'word_segment_forbidden_common_symbols',
  categories.word_segment_forbidden_common_symbols,
);
const eventBase = buildCharClass(
  'event_segment_forbidden_symbols',
  categories.event_segment_forbidden_symbols,
);
const eventCommon = buildCharClass(
  'event_segment_forbidden_common_symbols',
  categories.event_segment_forbidden_common_symbols,
);

const generated = `/**
 * Generated file from resources/spec/symbols/symbol_registry.json
 *
 * DO NOT EDIT MANUALLY.
 * To regenerate:
 *   cd talkbank-tools && node scripts/generate-symbol-sets.js
 */

export const CA_DELIMITER_SYMBOLS = String.raw\`${escapeTemplateLiteral(delimiter)}\`;
export const CA_ELEMENT_SYMBOLS = String.raw\`${escapeTemplateLiteral(element)}\`;
export const CA_ALL_SYMBOLS = String.raw\`${escapeTemplateLiteral(all)}\`;

export const WORD_SEGMENT_FORBIDDEN_START_BASE = ${JSON.stringify(wordStartBase)};
export const WORD_SEGMENT_FORBIDDEN_REST_BASE = ${JSON.stringify(wordRestBase)};
export const WORD_SEGMENT_FORBIDDEN_COMMON = ${JSON.stringify(wordCommon)};

export const EVENT_SEGMENT_FORBIDDEN_BASE = ${JSON.stringify(eventBase)};
export const EVENT_SEGMENT_FORBIDDEN_COMMON = ${JSON.stringify(eventCommon)};
`;

const existing = fs.existsSync(outputPath) ? fs.readFileSync(outputPath, 'utf8') : null;
if (existing !== generated) {
  fs.writeFileSync(outputPath, generated, 'utf8');
}
