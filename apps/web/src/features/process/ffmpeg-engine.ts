import { FFFSType, FFmpeg } from '@ffmpeg/ffmpeg'
import type { ThumbnailFrame } from '@luna-web/core'
import { THUMBNAIL_POSITIONS } from '@luna-web/core'
import { cachedBlobUrl } from '@/lib/engine-cache'

// @ffmpeg/core's package.json has no "./package.json" export, so
// `import ... from '@ffmpeg/core/package.json' with { type: 'json' }` is
// blocked by Node/bundler exports-map resolution (verified: Node raises
// ERR_PACKAGE_PATH_NOT_EXPORTED for that subpath). This constant mirrors the
// @ffmpeg/core devDependency version pinned in package.json/bun.lock — bump
// it alongside that dependency.
const FFMPEG_CORE_VERSION = '0.12.10'

const CORE_BASE = `https://cdn.jsdelivr.net/npm/@ffmpeg/core@${FFMPEG_CORE_VERSION}/dist/esm`
const MOUNT_DIR = '/work'

export interface FfmpegEngine {
  thumbnails(file: File, timestamps: number[], width: number): Promise<ThumbnailFrame<Blob>[]>
  dispose(): void
}

const DISPOSED_MESSAGE = 'ffmpeg engine disposed'

export function createFfmpegEngine(): FfmpegEngine {
  const ffmpeg = new FFmpeg()
  let loaded: Promise<void> | null = null
  let disposed = false

  function ensureLoaded(): Promise<void> {
    loaded ??= (async () => {
      const [coreURL, wasmURL] = await Promise.all([
        cachedBlobUrl(`${CORE_BASE}/ffmpeg-core.js`, 'text/javascript'),
        cachedBlobUrl(`${CORE_BASE}/ffmpeg-core.wasm`, 'application/wasm'),
      ])
      try {
        // The download window above is where a cancelled lane is disposed
        // (widest on a cold cache, where every lane waits on one ~31 MB
        // single-flighted fetch). dispose() could not terminate a worker that
        // did not exist yet, so bailing out here is what stops load() from
        // creating one on an engine nobody holds a reference to.
        if (disposed) throw new Error(DISPOSED_MESSAGE)
        await ffmpeg.load({ coreURL, wasmURL })
        // dispose() landed inside load(): its terminate() hit the half-built
        // instance, so terminate the worker load() has just finished creating.
        if (disposed) {
          ffmpeg.terminate()
          throw new Error(DISPOSED_MESSAGE)
        }
      } finally {
        // The engine has consumed the binaries; drop the ~31MB object URLs.
        // Also on the abandoned paths above, or two of them leak per lane.
        URL.revokeObjectURL(coreURL)
        URL.revokeObjectURL(wasmURL)
      }
    })().catch((err) => {
      // A transient fetch/load failure must not permanently brick this engine:
      // clear the cached promise so the next call retries. Disposal is not
      // transient — keep its rejection cached so a retry cannot re-download.
      if (!disposed) loaded = null
      throw err
    })
    return loaded
  }

  return {
    async thumbnails(file, timestamps, width) {
      await ensureLoaded()
      // Best-effort teardown of any leftovers from a previously-failed
      // cleanup, then (re)create the mount dir. A genuine failure here
      // surfaces on mount() below, inside the try, as a normal rejection
      // the pool's retry handles. One instance handles one clip at a time
      // (its pool lane) — a single instance is not concurrency-safe;
      // parallelism comes from multiple lanes, each with its own engine.
      await ffmpeg.unmount(MOUNT_DIR).catch(() => {})
      await ffmpeg.deleteDir(MOUNT_DIR).catch(() => {})
      await ffmpeg.createDir(MOUNT_DIR)
      // WORKERFS: ffmpeg reads the File lazily by range — no whole-file copy.
      await ffmpeg.mount(FFFSType.WORKERFS, { files: [file] }, MOUNT_DIR)
      try {
        const frames: ThumbnailFrame<Blob>[] = []
        for (let i = 0; i < timestamps.length; i++) {
          const t = timestamps[i] ?? 0
          const positionRatio = THUMBNAIL_POSITIONS[i] ?? THUMBNAIL_POSITIONS[0] ?? 0.1
          const out = `frame-${i}.jpg`
          const code = await ffmpeg.exec([
            '-ss',
            String(t),
            '-i',
            `${MOUNT_DIR}/${file.name}`,
            '-frames:v',
            '1',
            '-vf',
            `scale=${width}:-2`,
            '-q:v',
            '3',
            out,
          ])
          if (code !== 0) {
            await ffmpeg.deleteFile(out).catch(() => {})
            frames.push({ positionRatio, timestampSeconds: t, outcome: 'SeekFailed' })
            continue
          }
          const data = await ffmpeg.readFile(out)
          await ffmpeg.deleteFile(out)
          const bytes = typeof data === 'string' ? new TextEncoder().encode(data) : data
          if (bytes.length === 0) {
            frames.push({ positionRatio, timestampSeconds: t, outcome: 'DecodeFailed' })
            continue
          }
          frames.push({
            positionRatio,
            timestampSeconds: t,
            image: new Blob([bytes as BlobPart], { type: 'image/jpeg' }),
            mime: 'image/jpeg',
            outcome: 'Success',
          })
        }
        return frames
      } finally {
        await ffmpeg.unmount(MOUNT_DIR).catch(() => {})
        await ffmpeg.deleteDir(MOUNT_DIR).catch(() => {})
      }
    },
    // Sticky by necessity: @ffmpeg/ffmpeg's terminate() is a no-op until
    // load() has created the worker, and it records nothing — a terminated
    // instance loads again quite happily. The flag is the only durable mark
    // that this engine was thrown away.
    dispose() {
      disposed = true
      ffmpeg.terminate()
    },
  }
}
