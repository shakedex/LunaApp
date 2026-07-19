import { Document, Image, Link, Page, StyleSheet, Text, View } from '@react-pdf/renderer'
import { formatBytes, formatDuration } from '@/lib/format'
import { GEIST, GEIST_MONO } from './pdf-fonts'
import {
  breakablePath,
  cameraFacts,
  type Fact,
  fileFacts,
  joinPath,
  reelPath,
  videoFacts,
} from './pdf-format'
import type { PdfClip, PdfReel, PdfReport } from './pdf-prepare'

// Spec §3 palette — the app's Cinema Dark tokens, hex-resolved.
const C = {
  page: '#151519',
  band: '#1D1D22',
  text: '#E7E7EA',
  muted: '#9EA0A8',
  accent: '#9AD6F2',
} as const

const LUNA_URL = 'https://luna.ozer2.one'

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

  reelSection: { marginBottom: 12 },
  reelHeader: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'baseline',
    marginBottom: 4,
  },
  reelName: {
    color: C.accent,
    fontSize: 12,
    fontWeight: 600,
    flexGrow: 1,
    flexShrink: 1,
    flexBasis: 0,
    paddingRight: 12,
  },
  reelPath: { color: C.muted, fontFamily: GEIST_MONO, fontSize: 7, fontWeight: 400 },
  reelMeta: { color: C.muted, fontFamily: GEIST_MONO, fontSize: 7, flexShrink: 0 },

  band: { flexDirection: 'row', paddingVertical: 8, paddingHorizontal: 10 },
  bandAlt: { backgroundColor: C.band },
  clipName: { fontSize: 10, fontWeight: 600, marginBottom: 3 },
  clipPath: { color: C.muted, fontFamily: GEIST_MONO, fontSize: 7, marginTop: 3 },
  bandLeft: { width: '40%', paddingRight: 10 },
  bandRight: {
    width: '60%',
    flexDirection: 'row',
    justifyContent: 'flex-end',
    gap: 4,
  },
  factLine: { marginBottom: 2, color: C.text },
  factLabel: { color: C.muted },
  timecode: { fontFamily: GEIST_MONO, marginBottom: 2 },
  thumb: { width: 104, height: 58, objectFit: 'contain' },

  otherFilesEyebrow: {
    color: C.muted,
    fontSize: 7,
    letterSpacing: 2,
    marginTop: 6,
    marginBottom: 3,
    paddingHorizontal: 10,
  },
  otherFileName: { fontSize: 9, fontWeight: 500, marginBottom: 2 },
  otherFileSize: { fontFamily: GEIST_MONO, color: C.muted, fontSize: 8 },

  footerLeft: {
    position: 'absolute',
    bottom: 20,
    left: 28,
    color: C.muted,
    fontSize: 7,
  },
  footerLink: { color: C.muted, textDecoration: 'none' },
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

function FactLine({ facts }: { facts: Fact[] }) {
  if (facts.length === 0) return null
  return (
    <Text style={styles.factLine}>
      {facts.map((f, i) => (
        <Text key={`${f.label ?? ''}${f.value}`}>
          {i > 0 ? SEP : ''}
          {f.label ? <Text style={styles.factLabel}>{f.label} </Text> : null}
          <Text style={f.mono ? styles.mono : undefined}>{f.value}</Text>
        </Text>
      ))}
    </Text>
  )
}

function ClipBand({ clip, index, root }: { clip: PdfClip; index: number; root: string }) {
  const frames = clip.frames.filter((f) => f.dataUrl !== null)
  return (
    <View style={index % 2 === 0 ? [styles.band, styles.bandAlt] : styles.band} wrap={false}>
      <View style={frames.length > 0 ? styles.bandLeft : { width: '100%' }}>
        <Text style={styles.clipName}>{clip.fileName}</Text>
        <FactLine facts={fileFacts(clip)} />
        <FactLine facts={videoFacts(clip.metadata)} />
        {clip.metadata.startTimecode ? (
          <Text style={styles.timecode}>
            <Text style={styles.factLabel}>TC </Text>
            {clip.metadata.startTimecode}
          </Text>
        ) : null}
        <FactLine facts={cameraFacts(clip.metadata)} />
        <Text style={styles.clipPath}>{breakablePath(joinPath(root, clip.relativePath))}</Text>
      </View>
      {frames.length > 0 ? (
        <View style={styles.bandRight}>
          {frames.map((frame, i) => (
            <Image key={String(i)} src={frame.dataUrl as string} style={styles.thumb} />
          ))}
        </View>
      ) : null}
    </View>
  )
}

function ReelSection({ reel, root }: { reel: PdfReel; root: string }) {
  const path = reelPath(root, reel)
  const meta = [
    `${reel.stats.clipCount} ${reel.stats.clipCount === 1 ? 'clip' : 'clips'}`,
    reel.stats.otherFileCount > 0
      ? `${reel.stats.otherFileCount} other ${
          reel.stats.otherFileCount === 1 ? 'file' : 'files'
        } (${formatBytes(reel.stats.otherFileSizeBytes)})`
      : null,
    formatDuration(reel.stats.totalDurationSeconds),
    formatBytes(reel.stats.totalSizeBytes),
  ]
    .filter(Boolean)
    .join(SEP)
  return (
    <View style={styles.reelSection}>
      <View style={styles.reelHeader}>
        <Text style={styles.reelName}>
          {reel.name}
          {path ? (
            <Text style={styles.reelPath}>
              {'   '}
              {breakablePath(path)}
            </Text>
          ) : null}
        </Text>
        <Text style={styles.reelMeta}>{meta}</Text>
      </View>
      {reel.clips.map((clip, i) => (
        <ClipBand key={clip.relativePath} clip={clip} index={i} root={root} />
      ))}
      {reel.otherFiles.length > 0 ? (
        <View>
          <Text style={styles.otherFilesEyebrow}>OTHER FILES</Text>
          {reel.otherFiles.map((f, i) => (
            <View
              key={f.relativePath}
              style={i % 2 === 0 ? [styles.band, styles.bandAlt] : styles.band}
              wrap={false}
            >
              <View style={{ flexGrow: 1, paddingRight: 10 }}>
                <Text style={styles.otherFileName}>{f.fileName}</Text>
                <Text style={styles.clipPath}>{breakablePath(joinPath(root, f.relativePath))}</Text>
              </View>
              <Text style={styles.otherFileSize}>{formatBytes(f.sizeBytes)}</Text>
            </View>
          ))}
        </View>
      ) : null}
    </View>
  )
}

export function ReportDocument({ report }: { report: PdfReport }) {
  const { cover, stats, sourceRoot } = report
  // Local wall-clock stamp, second precision — the report doubles as a log entry.
  const now = new Date()
  const p2 = (n: number) => String(n).padStart(2, '0')
  const generated =
    `${now.getFullYear()}-${p2(now.getMonth() + 1)}-${p2(now.getDate())}` +
    ` ${p2(now.getHours())}:${p2(now.getMinutes())}:${p2(now.getSeconds())}`
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

        {report.reels.map((reel) => (
          <ReelSection key={reel.name} reel={reel} root={sourceRoot} />
        ))}

        <Text style={styles.footerLeft} fixed>
          Generated by{' '}
          <Link src={LUNA_URL} style={styles.footerLink}>
            Luna
          </Link>
          <Text style={styles.mono}>
            {SEP}
            {generated}
          </Text>
        </Text>
        <Text
          style={styles.footerRight}
          fixed
          render={({ pageNumber, totalPages }) => `${title}${SEP}${pageNumber}/${totalPages}`}
        />
      </Page>
    </Document>
  )
}
