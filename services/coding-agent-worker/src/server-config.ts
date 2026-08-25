/** Explicit worker-owned server port configuration. */
export function resolveWorkerServerPort(config: {
  SDK_WEBHOOK_HOST_RECEIVER_PORT: number;
}): number {
  return config.SDK_WEBHOOK_HOST_RECEIVER_PORT;
}
