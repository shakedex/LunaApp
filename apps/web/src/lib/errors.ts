/** The one way to turn a caught unknown into a display/log string. */
export function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err)
}
