/**
 * Vitest configuration for VS Code extension tests.
 *
 * Configures test environment, coverage, and file patterns.
 */

import { defineConfig } from 'vitest/config';
import path from 'path';

export default defineConfig({
    test: {
        // Use Node.js environment (not jsdom) since we're testing VS Code extension code
        environment: 'node',

        // Test file patterns
        include: ['src/test/**/*.test.ts'],
        exclude: ['node_modules', 'out', '.vscode-test'],

        // Enable globals (describe, it, expect without imports)
        globals: true,

        // Coverage configuration
        coverage: {
            provider: 'v8',
            reporter: ['text', 'html', 'lcov'],
            include: ['src/**/*.ts'],
            exclude: [
                'src/test/**',
                'src/**/*.test.ts',
                'src/**/*.spec.ts',
                'src/test/**'
            ],
            // Aim for high coverage since we're following TDD
            lines: 80,
            functions: 80,
            branches: 75,
            statements: 80
        },

        // Test timeout (increase for slow operations)
        testTimeout: 10000,

        // Reporter configuration
        reporters: ['verbose'],

        // Mock configuration
        mockReset: true,
        clearMocks: true,
        restoreMocks: true
    },

    // Resolve configuration for TypeScript paths
    resolve: {
        alias: {
            '@': path.resolve(__dirname, './src')
        }
    }
});
