import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createCommandSpecificMock } from './mocks/childProcess';
import { ExecutableService } from '../executableService';
import {
    parseValidationResult,
    ValidationExecutor,
    ValidationStatus
} from '../validation/executor';

describe('parseValidationResult', () => {
    it('parses valid results', () => {
        const result = parseValidationResult(
            '/tmp/example.cha',
            JSON.stringify({ status: 'valid' })
        );

        expect(result).toEqual({
            path: '/tmp/example.cha',
            status: ValidationStatus.Valid,
            isCacheHit: false
        });
    });

    it('parses invalid results with errors', () => {
        const result = parseValidationResult(
            '/tmp/example.cha',
            JSON.stringify({
                status: 'invalid',
                errors: [
                    {
                        code: 'E001',
                        severity: 'error',
                        message: 'Bad thing',
                        line: 4,
                        column: 2
                    }
                ]
            })
        );

        expect(result.status).toBe(ValidationStatus.Invalid);
        expect(result.errorCount).toBe(1);
        expect(result.errors?.[0]?.code).toBe('E001');
    });

    it('parses parse_error and read_error statuses', () => {
        expect(
            parseValidationResult(
                '/tmp/example.cha',
                JSON.stringify({ status: 'parse_error' })
            ).status
        ).toBe(ValidationStatus.ParseError);

        expect(
            parseValidationResult(
                '/tmp/example.cha',
                JSON.stringify({ status: 'read_error' })
            ).status
        ).toBe(ValidationStatus.ReadError);
    });

    it('throws on unknown status', () => {
        expect(() =>
            parseValidationResult(
                '/tmp/example.cha',
                JSON.stringify({ status: 'mystery' })
            )
        ).toThrow('Unknown validation status: mystery');
    });
});

describe('ValidationExecutor', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it('runs validate with the talkbank CLI', async () => {
        const execSync = createCommandSpecificMock({
            'validate "/tmp/example.cha" --json': {
                stdout: JSON.stringify({ status: 'valid' })
            }
        });
        const executor = new ValidationExecutor({
            executableService: new ExecutableService({ execSync }),
            locateCli: async () => '/tmp/talkbank'
        });

        const result = await executor.validateFile('/tmp/example.cha');

        expect(result.status).toBe(ValidationStatus.Valid);
        expect(execSync).toHaveBeenCalledWith(
            '"/tmp/talkbank" validate "/tmp/example.cha" --json',
            expect.objectContaining({
                encoding: 'utf-8',
                stdio: ['pipe', 'pipe', 'pipe']
            })
        );
    });

    it('uses dry-run before regular cache clear commands', async () => {
        const execSync = createCommandSpecificMock({
            'cache clear --prefix "/tmp/corpus" --dry-run': {
                stdout: 'Would clear 2 entries'
            },
            'cache clear --prefix "/tmp/corpus"': {
                stdout: 'Cleared 2 entries'
            }
        });
        const executor = new ValidationExecutor({
            executableService: new ExecutableService({ execSync }),
            locateCli: async () => '/tmp/talkbank'
        });

        await expect(executor.previewCacheClear('/tmp/corpus')).resolves.toBe(
            'Would clear 2 entries'
        );
        await expect(executor.clearCache('/tmp/corpus')).resolves.toBe(
            'Cleared 2 entries'
        );
    });

    it('throws a clear error when the CLI cannot be located', async () => {
        const executor = new ValidationExecutor({
            executableService: new ExecutableService({
                execSync: createCommandSpecificMock({}),
            }),
            locateCli: async () => null
        });

        await expect(executor.validateFile('/tmp/example.cha')).rejects.toThrow(
            'chatter CLI not found. Please build the project first.'
        );
    });
});
