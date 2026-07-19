// .woff, not .woff2: react-pdf's fontkit-based WOFF2 subsetter throws
// "RangeError: Out of bounds access" while embedding these Geist faces the
// moment the document contains a middle dot (·, used throughout as a
// separator) or the zero-width space breakablePath() inserts after every
// '/'. No .ttf ships in this @fontsource package version to use as the
// originally-planned fallback; .woff is the equivalent supported format
// (@react-pdf/font reads TTF/WOFF/WOFF2 alike) and does not hit the bug.
import geist400 from '@fontsource/geist/files/geist-latin-400-normal.woff'
import geist500 from '@fontsource/geist/files/geist-latin-500-normal.woff'
import geist600 from '@fontsource/geist/files/geist-latin-600-normal.woff'
import geistMono400 from '@fontsource/geist-mono/files/geist-mono-latin-400-normal.woff'
import geistMono500 from '@fontsource/geist-mono/files/geist-mono-latin-500-normal.woff'
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

// Spec §3: no mid-word hyphenation, ever. breakablePath() marks legal break
// points with zero-width spaces after each '/'; splitting on them here makes
// those the only break opportunities — textkit then wraps at segment
// boundaries without appending the "-" glyph it adds to forced mid-word
// breaks (and the ZWSP itself never reaches the page).
Font.registerHyphenationCallback((word) => word.split('\u200B'))
