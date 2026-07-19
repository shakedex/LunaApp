import { Document, Image, Page, StyleSheet, Text, View } from '@react-pdf/renderer'
import { formatBytes, formatDuration } from '@/lib/format'
import type { PdfClip, PdfReport } from './pdf-prepare'

const styles = StyleSheet.create({
  page: { padding: 36, fontSize: 9, fontFamily: 'Helvetica', color: '#111' },
  coverRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    marginBottom: 12,
  },
  title: { fontSize: 20, fontFamily: 'Helvetica-Bold' },
  coverMeta: { color: '#555', marginTop: 2 },
  logo: { height: 32, objectFit: 'contain' },
  statsRow: {
    flexDirection: 'row',
    gap: 16,
    marginBottom: 16,
    paddingBottom: 8,
    borderBottomWidth: 1,
    borderBottomColor: '#ddd',
  },
  statValue: { fontSize: 14, fontFamily: 'Helvetica-Bold' },
  statLabel: { color: '#777' },
  reelHeader: { fontSize: 13, fontFamily: 'Helvetica-Bold', marginTop: 12, marginBottom: 6 },
  reelMeta: { color: '#777', fontSize: 9 },
  card: { borderWidth: 1, borderColor: '#ddd', borderRadius: 4, padding: 8, marginBottom: 8 },
  cardHeader: { flexDirection: 'row', justifyContent: 'space-between', marginBottom: 6 },
  fileName: { fontFamily: 'Helvetica-Bold' },
  timecode: { color: '#555' },
  frameRow: { flexDirection: 'row', gap: 4, marginBottom: 6 },
  frame: { width: 160, height: 90, objectFit: 'contain', backgroundColor: '#f2f2f2' },
  framePlaceholder: {
    width: 160,
    height: 90,
    backgroundColor: '#f2f2f2',
    alignItems: 'center',
    justifyContent: 'center',
    color: '#999',
  },
  metaColumns: { flexDirection: 'row', gap: 24 },
  metaColumn: { flexGrow: 1 },
  metaRow: { flexDirection: 'row', justifyContent: 'space-between', marginBottom: 1 },
  metaLabel: { color: '#777' },
  footer: {
    position: 'absolute',
    bottom: 18,
    left: 36,
    right: 36,
    textAlign: 'right',
    color: '#999',
    fontSize: 8,
  },
})

const LEFT_META = [
  [
    'Resolution',
    (c: PdfClip) =>
      c.metadata.width && c.metadata.height
        ? `${c.metadata.width}×${c.metadata.height}`
        : undefined,
  ],
  ['Codec', (c: PdfClip) => c.metadata.codec],
  [
    'Frame rate',
    (c: PdfClip) =>
      c.metadata.frameRate !== undefined ? `${c.metadata.frameRate} fps` : undefined,
  ],
  [
    'Duration',
    (c: PdfClip) =>
      c.metadata.durationSeconds !== undefined
        ? formatDuration(c.metadata.durationSeconds)
        : undefined,
  ],
  ['Size', (c: PdfClip) => formatBytes(c.sizeBytes)],
  ['Color space', (c: PdfClip) => c.metadata.colorSpace],
] as const

const RIGHT_META = [
  ['Camera', (c: PdfClip) => c.metadata.camera],
  ['ISO', (c: PdfClip) => c.metadata.iso],
  ['White balance', (c: PdfClip) => c.metadata.whiteBalance],
  ['Lens', (c: PdfClip) => c.metadata.lens],
  ['Focal length', (c: PdfClip) => c.metadata.focalLength],
  ['Aperture', (c: PdfClip) => c.metadata.aperture],
  ['Shutter', (c: PdfClip) => c.metadata.shutter],
  ['Gamma', (c: PdfClip) => c.metadata.gamma],
] as const

function ClipCard({ clip }: { clip: PdfClip }) {
  return (
    <View style={styles.card} wrap={false}>
      <View style={styles.cardHeader}>
        <Text style={styles.fileName}>{clip.fileName}</Text>
        {clip.metadata.startTimecode ? (
          <Text style={styles.timecode}>TC {clip.metadata.startTimecode}</Text>
        ) : null}
      </View>
      <View style={styles.frameRow}>
        {clip.frames.map((frame, i) =>
          frame.dataUrl ? (
            <Image key={String(i)} src={frame.dataUrl} style={styles.frame} />
          ) : (
            <View key={String(i)} style={styles.framePlaceholder}>
              <Text>no preview</Text>
            </View>
          ),
        )}
      </View>
      <View style={styles.metaColumns}>
        <View style={styles.metaColumn}>
          {LEFT_META.map(([label, get]) => (
            <View key={label} style={styles.metaRow}>
              <Text style={styles.metaLabel}>{label}</Text>
              <Text>{get(clip) ?? '—'}</Text>
            </View>
          ))}
        </View>
        <View style={styles.metaColumn}>
          {RIGHT_META.map(([label, get]) => (
            <View key={label} style={styles.metaRow}>
              <Text style={styles.metaLabel}>{label}</Text>
              <Text>{get(clip) ?? '—'}</Text>
            </View>
          ))}
        </View>
      </View>
    </View>
  )
}

export function ReportDocument({ report }: { report: PdfReport }) {
  const { cover, stats } = report
  return (
    <Document title={cover.projectTitle ?? 'Camera report'} creator="Luna Web">
      <Page size="A4" style={styles.page}>
        <View style={styles.coverRow}>
          <View>
            <Text style={styles.title}>{cover.projectTitle ?? 'Camera report'}</Text>
            <Text style={styles.coverMeta}>
              {[cover.productionCompany, cover.date].filter(Boolean).join(' · ')}
            </Text>
            <Text style={styles.coverMeta}>
              {[
                cover.dit ? `DIT ${cover.dit}` : undefined,
                cover.director ? `Dir ${cover.director}` : undefined,
                cover.dp ? `DP ${cover.dp}` : undefined,
              ]
                .filter(Boolean)
                .join(' · ')}
            </Text>
          </View>
          {cover.logoDataUrl ? <Image src={cover.logoDataUrl} style={styles.logo} /> : null}
        </View>

        <View style={styles.statsRow}>
          <View>
            <Text style={styles.statValue}>{stats.cardCount}</Text>
            <Text style={styles.statLabel}>Cards</Text>
          </View>
          <View>
            <Text style={styles.statValue}>{stats.clipCount}</Text>
            <Text style={styles.statLabel}>Clips</Text>
          </View>
          <View>
            <Text style={styles.statValue}>{formatDuration(stats.totalDurationSeconds)}</Text>
            <Text style={styles.statLabel}>Duration</Text>
          </View>
          <View>
            <Text style={styles.statValue}>{formatBytes(stats.totalSizeBytes)}</Text>
            <Text style={styles.statLabel}>Size</Text>
          </View>
          <View>
            <Text style={styles.statValue}>{report.rawCount}</Text>
            <Text style={styles.statLabel}>RAW</Text>
          </View>
        </View>

        {report.reels.map((reel) => (
          <View key={reel.name}>
            <Text style={styles.reelHeader}>
              {reel.name}
              {'  '}
              <Text style={styles.reelMeta}>
                {reel.stats.clipCount} clips · {formatBytes(reel.stats.totalSizeBytes)} ·{' '}
                {formatDuration(reel.stats.totalDurationSeconds)}
              </Text>
            </Text>
            {reel.clips.map((clip) => (
              <ClipCard key={clip.relativePath} clip={clip} />
            ))}
          </View>
        ))}

        <Text
          style={styles.footer}
          fixed
          render={({ pageNumber, totalPages }) =>
            `Generated by Luna Web · ${pageNumber}/${totalPages}`
          }
        />
      </Page>
    </Document>
  )
}
