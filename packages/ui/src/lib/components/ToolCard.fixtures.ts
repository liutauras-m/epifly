import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'ToolCard',
  note: 'Expandable <details>-based tool call card showing name, status dot, elapsed time, and result body.',
  cases: [
    {
      label: 'Running',
      props: {
        id: 'tc-running',
        name: 'web_search__query',
        status: 'running',
        result: '',
        startTime: Date.now(),
      },
    },
    {
      label: 'Success',
      props: {
        id: 'tc-success',
        name: 'read_file',
        status: 'success',
        result: '{"lines": 42, "encoding": "utf-8"}',
        startTime: Date.now() - 1230,
      },
    },
    {
      label: 'Error with retry',
      props: {
        id: 'tc-error',
        name: 'execute_command',
        status: 'error',
        result: 'Error: command not found: foobar',
        startTime: Date.now() - 450,
        onRetry: () => {},
      },
    },
    {
      label: 'Compound name',
      props: {
        id: 'tc-compound',
        name: 'media_time__get_current_time',
        status: 'success',
        result: '14:32:07 UTC',
        startTime: Date.now() - 88,
      },
    },
  ],
};

export default fixtures;
