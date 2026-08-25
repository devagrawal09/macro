import { defineConfig } from '@hey-api/openapi-ts';
import { services, type ServiceSpec } from './services';

const requestedService = process.env.SDK_GENERATE_SERVICE;
if (requestedService && !services.includes(requestedService as ServiceSpec)) {
  throw new Error(`unknown SDK service ${requestedService}`);
}
const selectedServices = requestedService
  ? services.filter((service) => service === requestedService)
  : services;

/**
 * hey-api config for the SDK's generated layer.
 *
 * Everything emitted goes under ./generated/ — treat that folder as build
 * output (do not hand-edit; it is overwritten on regenerate). The hand-written
 * SDK code lives in ./src and imports from ./generated/.
 *
 * One entry per service. Each service produces:
 *   - ./generated/<service>/sdk.gen.ts        (typed SDK functions)
 *   - ./generated/<service>/types.gen.ts      (generated TS models)
 *   - ./generated/<service>/client.gen.ts     (default client instance)
 *   - ./generated/<service>/client/           (client implementation, self-contained)
 *
 * Input specs live in ./specs/<service>.json, refreshed from the monorepo's
 * service-clients package by `bun run sync-specs` (or end-to-end from the
 * Rust services with `just update-generated`).
 */
export default defineConfig(
  selectedServices.map((service) => ({
    input: `./specs/${service}.json`,
    output: {
      path: `./generated/${service}`,
      postProcess: [
        {
          command: process.execPath,
          args: ['scripts/format-generated.mjs', '{{path}}'],
        },
      ],
    },
    plugins: [
      '@hey-api/client-fetch',
      '@hey-api/typescript',
      {
        name: '@hey-api/sdk',
        operations: { strategy: 'single', methods: 'instance' },
      },
    ],
  })),
);
