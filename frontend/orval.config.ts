import { defineConfig } from 'orval';

export default defineConfig({
  acpms: {
    input: {
      target: 'http://localhost:3000/api-docs/openapi.json',
    },
    output: {
      mode: 'tags-split',
      target: 'src/api/generated',
      client: 'react-query',
      schemas: 'src/api/generated/models',
      override: {
        mutator: {
          path: 'src/api/client.ts',
          name: 'customFetch',
        },
        query: {
          useQuery: true,
          useMutation: true,
        },
      },
    },
  },
});
