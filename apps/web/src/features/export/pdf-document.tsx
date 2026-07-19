import { Document, Image, Page, StyleSheet, Text, View } from '@react-pdf/renderer'
import { formatBytes, formatDuration } from '@/lib/format'
import { GEIST, GEIST_MONO } from './pdf-fonts'
import { breakablePath, joinPath, reelPath } from './pdf-format'
import type { PdfClip, PdfReel, PdfReport } from './pdf-prepare'

// Spec §3 palette — the app's Cinema Dark tokens, hex-resolved.
const C = {
  page: '#151519',
  band: '#1D1D22',
  pathBand: '#26262C',
  text: '#E7E7EA',
  muted: '#9EA0A8',
  accent: '#9AD6F2',
} as const

const styles = StyleSheet.create({
  page: {
    backgroundColor: C.page,
    color: C.text,
    fontFamily: GEIST,
    fontSize: 8,
    paddingTop: 28,
    paddingHorizontal: 28,
    paddingBottom: 44,
  },
  mono: { fontFamily: GEIST_MONO },

  headerRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'flex-start',
    marginBottom: 10,
  },
  eyebrow: { color: C.muted, fontSize: 7, letterSpacing: 2, marginBottom: 4 },
  title: { fontSize: 20, fontWeight: 600 },
  coverMeta: { color: C.muted, marginTop: 3, fontSize: 8 },
  coverMetaValue: { color: C.text },
  logo: { maxHeight: 32, maxWidth: 140, objectFit: 'contain' },

  totals: { marginBottom: 10 },
  totalValue: { fontFamily: GEIST_MONO, fontWeight: 500 },
  totalLabel: { color: C.muted },

  pathBand: {
    backgroundColor: C.pathBand,
    color: C.muted,
    fontFamily: GEIST_MONO,
    fontSize: 7,
    paddingVertical: 4,
    paddingHorizontal: 10,
    marginBottom: 8,
  },

  reelSection: { marginBottom: 12 },
  reelHeader: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'baseline',
    marginBottom: 4,
  },
  reelName: { color: C.accent, fontSize: 12, fontWeight: 600 },
  reelMeta: { color: C.muted, fontFamily: GEIST_MONO, fontSize: 7 },

  band: { flexDirection: 'row', paddingVertical: 8, paddingHorizontal: 10 },
  bandAlt: { backgroundColor: C.band },
  clipName: { fontSize: 10, fontWeight: 600, marginBottom: 3 },
  clipPath: { color: C.muted, fontFamily: GEIST_MONO, fontSize: 7, marginTop: 3 },

  footerLeft: {
    position: 'absolute',
    bottom: 20,
    left: 28,
    color: C.muted,
    fontSize: 7,
  },
  footerRight: {
    position: 'absolute',
    bottom: 20,
    right: 28,
    color: C.muted,
    fontFamily: GEIST_MONO,
    fontSize: 7,
  },
})

const SEP = '  ·  '

/** Temporary minimal band — Task 4 replaces this component and nothing else. */
function ClipBand({ clip, index, root }: { clip: PdfClip; index: number; root: string }) {
  return (
    <View style={index % 2 === 0 ? [styles.band, styles.bandAlt] : styles.band} wrap={false}>
      <View style={{ width: '100%' }}>
        <Text style={styles.clipName}>{clip.fileName}</Text>
        <Text style={styles.mono}>{formatBytes(clip.sizeBytes)}</Text>
        <Text style={styles.clipPath}>{breakablePath(joinPath(root, clip.relativePath))}</Text>
      </View>
    </View>
  )
}

function ReelSection({ reel, root }: { reel: PdfReel; root: string }) {
  const path = reelPath(root, reel)
  const meta = [
    `${reel.stats.clipCount} clips`,
    reel.stats.otherFileCount > 0
      ? `${reel.stats.otherFileCount} other files (${formatBytes(reel.stats.otherFileSizeBytes)})`
      : null,
    formatDuration(reel.stats.totalDurationSeconds),
    formatBytes(reel.stats.totalSizeBytes),
  ]
    .filter(Boolean)
    .join(SEP)
  return (
    <View style={styles.reelSection}>
      <View style={styles.reelHeader}>
        <Text style={styles.reelName}>{reel.name}</Text>
        <Text style={styles.reelMeta}>{meta}</Text>
      </View>
      {path ? <Text style={styles.pathBand}>{breakablePath(path)}</Text> : null}
      {reel.clips.map((clip, i) => (
        <ClipBand key={clip.relativePath} clip={clip} index={i} root={root} />
      ))}
    </View>
  )
}

export function ReportDocument({ report }: { report: PdfReport }) {
  const { cover, stats, sourceRoot } = report
  const generated = new Date().toISOString().slice(0, 10)
  const title = cover.projectTitle || 'Camera report'

  const crewLine = [
    cover.dit ? ['DIT', cover.dit] : null,
    cover.director ? ['Director', cover.director] : null,
    cover.dp ? ['DP', cover.dp] : null,
  ].filter((x): x is [string, string] => x !== null)

  const totals: [string, string][] = [
    [String(stats.cardCount), stats.cardCount === 1 ? 'card' : 'cards'],
    [String(stats.clipCount), stats.clipCount === 1 ? 'clip' : 'clips'],
    ...(stats.otherFileCount > 0
      ? [
          [`${stats.otherFileCount} (${formatBytes(stats.otherFileSizeBytes)})`, 'other files'] as [
            string,
            string,
          ],
        ]
      : []),
    [formatDuration(stats.totalDurationSeconds), 'duration'],
    [formatBytes(stats.totalSizeBytes), 'total size'],
  ]

  return (
    <Document title={title} creator="Luna">
      <Page size="A4" style={styles.page}>
        <View style={styles.headerRow}>
          <View style={{ maxWidth: '75%' }}>
            <Text style={styles.eyebrow}>CAMERA REPORT</Text>
            <Text style={styles.title}>{title}</Text>
            {(cover.productionCompany || cover.date) && (
              <Text style={styles.coverMeta}>
                {[cover.productionCompany, cover.date].filter(Boolean).join(SEP)}
              </Text>
            )}
            {crewLine.length > 0 && (
              <Text style={styles.coverMeta}>
                {crewLine.map(([role, name], i) => (
                  <Text key={role}>
                    {i > 0 ? SEP : ''}
                    {role} <Text style={styles.coverMetaValue}>{name}</Text>
                  </Text>
                ))}
              </Text>
            )}
          </View>
          {cover.logoDataUrl ? <Image src={cover.logoDataUrl} style={styles.logo} /> : null}
        </View>

        <Text style={styles.totals}>
          {totals.map(([value, label], i) => (
            <Text key={label}>
              {i > 0 ? SEP : ''}
              <Text style={styles.totalValue}>{value}</Text>
              <Text style={styles.totalLabel}> {label}</Text>
            </Text>
          ))}
        </Text>

        {sourceRoot ? <Text style={styles.pathBand}>{breakablePath(sourceRoot)}</Text> : null}

        {report.reels.map((reel) => (
          <ReelSection key={reel.name} reel={reel} root={sourceRoot} />
        ))}

        <Text style={styles.footerLeft} fixed>
          {title}
        </Text>
        <Text
          style={styles.footerRight}
          fixed
          render={({ pageNumber, totalPages }) =>
            `Luna${SEP}${generated}${SEP}${pageNumber}/${totalPages}`
          }
        />
      </Page>
    </Document>
  )
}
