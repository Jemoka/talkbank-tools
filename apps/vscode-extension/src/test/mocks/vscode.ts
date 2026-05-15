/**
 * Mock implementation of VS Code API for testing.
 *
 * This provides minimal mocks for the VS Code API types needed in tests.
 * Use Vitest's vi.fn() for creating spies and stubs as needed.
 */

import { vi } from 'vitest';

/**
 * Mock ExtensionContext for testing extension activation.
 */
export interface MockExtensionContext {
    subscriptions: any[];
    workspaceState: MockMemento;
    globalState: MockMemento;
    extensionPath: string;
    storagePath: string | undefined;
    globalStoragePath: string;
    logPath: string;
    extensionUri: any;
    environmentVariableCollection: any;
    extensionMode: number;
    storageUri: any;
    globalStorageUri: any;
    logUri: any;
    asAbsolutePath: (relativePath: string) => string;
    secrets: any;
    extension: any;
}

/**
 * Mock Memento (state storage) for testing.
 */
export interface MockMemento {
    get<T>(key: string, defaultValue?: T): T | undefined;
    update(key: string, value: any): Promise<void>;
    keys(): readonly string[];
}

/**
 * Mock StatusBarItem for testing status bar updates.
 */
export interface MockStatusBarItem {
    text: string;
    tooltip: string | undefined;
    command: string | undefined;
    show: ReturnType<typeof vi.fn>;
    hide: ReturnType<typeof vi.fn>;
    dispose: ReturnType<typeof vi.fn>;
}

/**
 * Mock TreeView for testing tree views.
 */
export interface MockTreeView<T> {
    reveal: ReturnType<typeof vi.fn>;
    dispose: ReturnType<typeof vi.fn>;
    onDidExpandElement: any;
    onDidCollapseElement: any;
    selection: T[];
    visible: boolean;
}

/**
 * Creates a mock ExtensionContext with default values.
 *
 * @param overrides - Optional overrides for specific properties
 * @returns Mock extension context suitable for testing
 *
 * @example
 * ```typescript
 * const context = createMockExtensionContext({
 *   extensionPath: '/test/path'
 * });
 * ```
 */
export function createMockExtensionContext(
    overrides?: Partial<MockExtensionContext>
): MockExtensionContext {
    const storage = new Map<string, any>();

    const defaultContext: MockExtensionContext = {
        subscriptions: [],
        workspaceState: {
            get: <T>(key: string, defaultValue?: T) => storage.get(key) ?? defaultValue,
            update: async (key: string, value: any) => {
                storage.set(key, value);
            },
            keys: () => Array.from(storage.keys())
        },
        globalState: {
            get: <T>(key: string, defaultValue?: T) => storage.get(key) ?? defaultValue,
            update: async (key: string, value: any) => {
                storage.set(key, value);
            },
            keys: () => Array.from(storage.keys())
        },
        extensionPath: '/mock/extension/path',
        storagePath: '/mock/storage',
        globalStoragePath: '/mock/global/storage',
        logPath: '/mock/logs',
        extensionUri: { fsPath: '/mock/extension' },
        environmentVariableCollection: {},
        extensionMode: 3, // ExtensionMode.Test
        storageUri: { fsPath: '/mock/storage' },
        globalStorageUri: { fsPath: '/mock/global/storage' },
        logUri: { fsPath: '/mock/logs' },
        asAbsolutePath: (relativePath: string) => `/mock/extension/${relativePath}`,
        secrets: {},
        extension: {}
    };

    return { ...defaultContext, ...overrides };
}

/**
 * Creates a mock StatusBarItem.
 *
 * @returns Mock status bar item with spy functions
 *
 * @example
 * ```typescript
 * const statusBar = createMockStatusBarItem();
 * statusBar.text = 'Test';
 * expect(statusBar.show).toHaveBeenCalled();
 * ```
 */
export function createMockStatusBarItem(): MockStatusBarItem {
    return {
        text: '',
        tooltip: undefined,
        command: undefined,
        show: vi.fn(),
        hide: vi.fn(),
        dispose: vi.fn()
    };
}

/**
 * Creates a mock TreeView.
 *
 * @returns Mock tree view with spy functions
 *
 * @example
 * ```typescript
 * const treeView = createMockTreeView<MyNode>();
 * await treeView.reveal(node);
 * expect(treeView.reveal).toHaveBeenCalledWith(node);
 * ```
 */
export function createMockTreeView<T>(): MockTreeView<T> {
    return {
        reveal: vi.fn(),
        dispose: vi.fn(),
        onDidExpandElement: {},
        onDidCollapseElement: {},
        selection: [],
        visible: true
    };
}
