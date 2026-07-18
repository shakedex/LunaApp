import { guardedUpdate } from './run-processing'

// Placeholder for Task 6: transitions the pipeline out of the processing
// phase. Task 6 replaces this body with the real thumbnail pass.
export async function startThumbnails(run: number): Promise<void> {
  guardedUpdate(run, (s) => ({ ...s, phase: 'processed' }))
}
