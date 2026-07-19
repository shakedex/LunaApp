import geist400 from '@fontsource/geist/files/geist-latin-400-normal.woff2'
import geist500 from '@fontsource/geist/files/geist-latin-500-normal.woff2'
import geist600 from '@fontsource/geist/files/geist-latin-600-normal.woff2'
import geistMono400 from '@fontsource/geist-mono/files/geist-mono-latin-400-normal.woff2'
import geistMono500 from '@fontsource/geist-mono/files/geist-mono-latin-500-normal.woff2'
import { Font } from '@react-pdf/renderer'

export const GEIST = 'Geist'
export const GEIST_MONO = 'Geist Mono'

Font.register({
  family: GEIST,
  fonts: [
    { src: geist400, fontWeight: 400 },
    { src: geist500, fontWeight: 500 },
    { src: geist600, fontWeight: 600 },
  ],
})

Font.register({
  family: GEIST_MONO,
  fonts: [
    { src: geistMono400, fontWeight: 400 },
    { src: geistMono500, fontWeight: 500 },
  ],
})

// Spec §3: no mid-word hyphenation, ever. Paths wrap at '/' via zero-width
// spaces inserted in pdf-format.ts, not via hyphenation.
Font.registerHyphenationCallback((word) => [word])
