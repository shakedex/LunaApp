import type { ReportClip } from '@luna-web/core'
import { useWindowVirtualizer } from '@tanstack/react-virtual'
import { useLayoutEffect, useRef, useState } from 'react'
import { ClipCard } from './clip-card'

// Below this count plain rendering is cheaper than absolute positioning.
const VIRTUALIZE_THRESHOLD = 15

export function VirtualClipList({
  clips,
  sourceRoot,
}: {
  clips: ReportClip<Blob>[]
  sourceRoot: string
}) {
  if (clips.length <= VIRTUALIZE_THRESHOLD) {
    return (
      <div className="grid gap-4">
        {clips.map((clip) => (
          <ClipCard key={clip.id} clip={clip} sourceRoot={sourceRoot} />
        ))}
      </div>
    )
  }
  return <WindowedClipList clips={clips} sourceRoot={sourceRoot} />
}

function WindowedClipList({
  clips,
  sourceRoot,
}: {
  clips: ReportClip<Blob>[]
  sourceRoot: string
}) {
  const listRef = useRef<HTMLDivElement>(null)
  const [scrollMargin, setScrollMargin] = useState(0)

  // The window virtualizer needs the list's offset from the top of the page.
  // Content above (cover form, other reels) can change height, so track it.
  useLayoutEffect(() => {
    const el = listRef.current
    if (!el) return
    const update = () => setScrollMargin(el.getBoundingClientRect().top + window.scrollY)
    update()
    const observer = new ResizeObserver(update)
    observer.observe(document.body)
    return () => observer.disconnect()
  }, [])

  const virtualizer = useWindowVirtualizer({
    count: clips.length,
    estimateSize: () => 480,
    overscan: 3,
    gap: 16,
    scrollMargin,
    getItemKey: (i) => clips[i]?.id ?? i,
  })

  return (
    <div ref={listRef} className="relative" style={{ height: virtualizer.getTotalSize() }}>
      {virtualizer.getVirtualItems().map((item) => (
        <div
          key={item.key}
          data-index={item.index}
          ref={virtualizer.measureElement}
          className="absolute inset-x-0 top-0"
          style={{ transform: `translateY(${item.start - virtualizer.options.scrollMargin}px)` }}
        >
          <ClipCard clip={clips[item.index]} sourceRoot={sourceRoot} />
        </div>
      ))}
    </div>
  )
}
