import { readFileSync } from 'fs';
import { resolve } from 'path';

const bundlerService = readFileSync(resolve(__dirname, '../../..', 'src/services/bundler-service.ts'), 'utf8');
const safeTransaction = readFileSync(resolve(__dirname, '../../..', 'src/services/safe-transaction.ts'), 'utf8');

describe('splitter endpoint removal', () => {
  it('does not define or request the retired splitter endpoint', () => {
    expect(bundlerService).not.toContain('/v1/splitter');
    expect(bundlerService).not.toContain('fetchSplitterInfo');
  });

  it('does not invoke splitter discovery from transaction construction', () => {
    expect(safeTransaction).not.toContain('fetchSplitterInfo');
    expect(safeTransaction).not.toContain('splitterDeployCallIfNeeded');
  });
});
