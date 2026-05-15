import * as vscode from 'vscode';
import * as path from 'path';
import {
    type FileValidationIssue,
    type FileValidationResult,
    ValidationExecutor,
    ValidationStatus
} from './validation/executor';
import { FileFinder } from './validation/fileFinder';

type ValidationTreeItemType = 'directory' | 'file' | 'error';
type Awaitable<T> = T | PromiseLike<T>;

export interface ValidationExplorerWindow {
    showErrorMessage(message: string): Awaitable<unknown>;
    showWarningMessage(message: string, ...items: string[]): Awaitable<string | undefined>;
    showInformationMessage(message: string): Awaitable<unknown>;
    withProgress<R>(
        options: vscode.ProgressOptions,
        task: (
            progress: vscode.Progress<{
                message?: string;
                increment?: number;
            }>,
            token: vscode.CancellationToken,
        ) => Awaitable<R>,
    ): Awaitable<R>;
}

/**
 * Construction-time dependencies for the validation explorer provider.
 */
export interface ValidationExplorerProviderOptions {
    workspaceRoot?: string;
    executor: ValidationExecutor;
    fileFinder: FileFinder;
    window: ValidationExplorerWindow;
}

/**
 * Tree item for the Validation Explorer view.
 */
class ValidationTreeItem extends vscode.TreeItem {
    constructor(
        label: string,
        collapsibleState: vscode.TreeItemCollapsibleState,
        public itemType: ValidationTreeItemType,
        public itemPath?: string,
        public validationResult?: FileValidationResult,
        public error?: FileValidationIssue
    ) {
        super(label, collapsibleState);
        this.setIconAndCommand();
    }

    private setIconAndCommand(): void {
        if (this.itemType === 'directory') {
            this.iconPath = new vscode.ThemeIcon('folder');
            this.contextValue = 'directory';
            return;
        }

        if (this.itemType === 'file') {
            if (this.validationResult) {
                switch (this.validationResult.status) {
                    case ValidationStatus.Valid:
                        this.iconPath = new vscode.ThemeIcon(
                            'check',
                            new vscode.ThemeColor('testing.iconPassed')
                        );
                        this.description = '✓ valid';
                        break;
                    case ValidationStatus.Invalid:
                        this.iconPath = new vscode.ThemeIcon(
                            'error',
                            new vscode.ThemeColor('testing.iconFailed')
                        );
                        this.description = `✗ ${this.validationResult.errorCount} errors`;
                        break;
                    case ValidationStatus.Cached:
                        this.iconPath = new vscode.ThemeIcon(
                            'check',
                            new vscode.ThemeColor('testing.iconPassed')
                        );
                        this.description = '✓ valid (cached)';
                        break;
                    case ValidationStatus.ParseError:
                        this.iconPath = new vscode.ThemeIcon(
                            'error',
                            new vscode.ThemeColor('testing.iconFailed')
                        );
                        this.description = '✗ parse error';
                        break;
                    case ValidationStatus.ReadError:
                        this.iconPath = new vscode.ThemeIcon(
                            'error',
                            new vscode.ThemeColor('testing.iconFailed')
                        );
                        this.description = '✗ read error';
                        break;
                    case ValidationStatus.Validating:
                        this.iconPath = new vscode.ThemeIcon('loading~spin');
                        this.description = '⟳ validating...';
                        break;
                }
            }

            this.command = {
                title: 'Open File',
                command: 'vscode.open',
                arguments: [vscode.Uri.file(this.itemPath || '')]
            };
            this.contextValue = 'file';
            return;
        }

        this.iconPath = new vscode.ThemeIcon(
            'error',
            new vscode.ThemeColor('errorForeground')
        );
        this.contextValue = 'error';

        if (!this.error) {
            return;
        }

        const line = Math.max(0, (this.error.line ?? 1) - 1);
        const column = Math.max(0, (this.error.column ?? 1) - 1);
        this.command = {
            title: 'Go to Error',
            command: 'vscode.open',
            arguments: [
                vscode.Uri.file(this.itemPath || ''),
                {
                    selection: new vscode.Range(
                        new vscode.Position(line, column),
                        new vscode.Position(line, column)
                    )
                } as vscode.TextDocumentShowOptions
            ]
        };
    }
}

/**
 * Provider for the Validation Explorer tree view.
 */
export class ValidationExplorerProvider
    implements vscode.TreeDataProvider<ValidationTreeItem> {
    private readonly _onDidChangeTreeData = new vscode.EventEmitter<
        ValidationTreeItem | undefined | null | void
    >();
    readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

    private readonly workspaceRoot: string | undefined;
    private readonly executor: ValidationExecutor;
    private readonly fileFinder: FileFinder;
    private readonly window: ValidationExplorerWindow;
    private readonly validationResults = new Map<string, FileValidationResult>();
    private readonly validatingFiles = new Set<string>();

    constructor(options: ValidationExplorerProviderOptions) {
        this.workspaceRoot = options.workspaceRoot;
        this.executor = options.executor;
        this.fileFinder = options.fileFinder;
        this.window = options.window;
    }

    /**
     * Refresh the tree view.
     */
    refresh(item?: ValidationTreeItem): void {
        this._onDidChangeTreeData.fire(item);
    }

    /**
     * Get tree item for a given element.
     */
    getTreeItem(element: ValidationTreeItem): vscode.TreeItem {
        return element;
    }

    /**
     * Get children of a tree item.
     */
    async getChildren(element?: ValidationTreeItem): Promise<ValidationTreeItem[]> {
        if (!this.workspaceRoot) {
            return [
                new ValidationTreeItem(
                    'No workspace open',
                    vscode.TreeItemCollapsibleState.None,
                    'directory'
                )
            ];
        }

        if (!element) {
            return this.getDirectoryItems(this.workspaceRoot);
        }

        if (element.itemType === 'directory' && element.itemPath) {
            return this.getDirectoryItems(element.itemPath);
        }

        if (element.itemType === 'file' && element.validationResult?.errors) {
            return element.validationResult.errors.map((error) =>
                new ValidationTreeItem(
                    `[${error.code}] ${error.message}`,
                    vscode.TreeItemCollapsibleState.None,
                    'error',
                    element.itemPath,
                    undefined,
                    error
                )
            );
        }

        return [];
    }

    /**
     * Validate a single file.
     */
    async validateFile(filePath: string): Promise<void> {
        this.validatingFiles.add(filePath);
        this.refresh();

        try {
            const result = await this.executor.validateFile(filePath);
            this.validationResults.set(filePath, result);
        } catch (error) {
            await this.window.showErrorMessage(
                `Failed to validate ${path.basename(filePath)}: ${error}`
            );
        } finally {
            this.validatingFiles.delete(filePath);
            this.refresh();
        }
    }

    /**
     * Validate all files in a directory recursively.
     */
    async validateDirectory(dirPath: string): Promise<void> {
        const files = await this.fileFinder.findChatFiles(dirPath);

        if (files.length === 0) {
            await this.window.showWarningMessage(`No .cha files found in ${dirPath}`);
            return;
        }

        await this.window.withProgress(
            {
                location: vscode.ProgressLocation.Notification,
                title: `Validating ${files.length} files...`,
                cancellable: true
            },
            async (progress, token) => {
                for (let i = 0; i < files.length; i++) {
                    if (token.isCancellationRequested) {
                        break;
                    }

                    const file = files[i];
                    this.validatingFiles.add(file);
                    this.refresh();

                    try {
                        const result = await this.executor.validateFile(file);
                        this.validationResults.set(file, result);
                    } catch (error) {
                        console.error(`Error validating ${file}:`, error);
                    }

                    this.validatingFiles.delete(file);
                    progress.report({ increment: 100 / files.length });
                }

                this.refresh();
            }
        );
    }

    /**
     * Clear cache for a file or directory.
     */
    async clearCache(itemPath?: string): Promise<void> {
        const targetPath = itemPath || this.workspaceRoot;
        if (!targetPath) {
            await this.window.showErrorMessage('No path selected');
            return;
        }

        try {
            const dryRunOutput = await this.executor.previewCacheClear(targetPath);
            const selection = await this.window.showWarningMessage(
                `${dryRunOutput}\n\nProceed with clearing cache for ${path.basename(
                    targetPath
                )}?`,
                'Clear Cache',
                'Cancel'
            );

            if (selection !== 'Clear Cache') {
                return;
            }

            const output = await this.executor.clearCache(targetPath);
            await this.window.showInformationMessage(output);
            this.refresh();
        } catch (error) {
            await this.window.showErrorMessage(`Failed to clear cache: ${error}`);
        }
    }

    private async getDirectoryItems(dirPath: string): Promise<ValidationTreeItem[]> {
        const entries = await this.fileFinder.getDirectoryEntries(dirPath);

        return entries.map((entry) => {
            if (entry.itemType === 'directory') {
                return new ValidationTreeItem(
                    entry.name,
                    vscode.TreeItemCollapsibleState.Collapsed,
                    'directory',
                    entry.fullPath
                );
            }

            return this.createFileItem(entry.fullPath, entry.name);
        });
    }

    private createFileItem(
        filePath: string,
        label: string = path.basename(filePath)
    ): ValidationTreeItem {
        const result = this.currentValidationResult(filePath);
        return new ValidationTreeItem(
            label,
            result.errors && result.errors.length > 0
                ? vscode.TreeItemCollapsibleState.Expanded
                : vscode.TreeItemCollapsibleState.None,
            'file',
            filePath,
            result
        );
    }

    private currentValidationResult(filePath: string): FileValidationResult {
        if (this.validatingFiles.has(filePath)) {
            return {
                path: filePath,
                status: ValidationStatus.Validating
            };
        }

        return (
            this.validationResults.get(filePath) || {
                path: filePath,
                status: ValidationStatus.Valid
            }
        );
    }
}
