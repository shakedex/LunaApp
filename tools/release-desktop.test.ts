import { describe, expect, test } from 'bun:test'
import { bumpVersionFiles } from './release-desktop'

const PKG = `{
  "name": "@luna-web/desktop",
  "version": "0.1.0",
  "private": true
}
`

const CARGO = `[package]
name = "app"
version = "0.1.0"
description = "Camera reports, generated entirely client-side."

[dependencies]
serde = { version = "1.0", features = ["derive"] }
`

const CONF = `{
  "productName": "Luna",
  "version": "0.1.0",
  "identifier": "com.lunaweb.desktop"
}
`

describe('bumpVersionFiles', () => {
  const out = bumpVersionFiles({ pkg: PKG, cargo: CARGO, conf: CONF }, '0.2.0')

  test('bumps the package.json version', () => {
    expect(out.pkg).toContain('"version": "0.2.0"')
  })

  test('bumps the tauri.conf.json version', () => {
    expect(out.conf).toContain('"version": "0.2.0"')
  })

  test('bumps the Cargo.toml package version', () => {
    expect(out.cargo).toContain('version = "0.2.0"')
  })

  test('does not touch a dependency version in Cargo.toml', () => {
    expect(out.cargo).toContain('serde = { version = "1.0", features = ["derive"] }')
  })

  test('leaves other fields alone', () => {
    expect(out.pkg).toContain('"name": "@luna-web/desktop"')
    expect(out.conf).toContain('"identifier": "com.lunaweb.desktop"')
    expect(out.cargo).toContain('name = "app"')
  })
})

// A [package] section using `version.workspace = true` (a normal Cargo workspace pattern) has no
// literal `version = "..."` line. The only literal `version = "..."` line in this fixture starts
// a line of its own inside a dependency's long-form table — the exact shape an unbounded scan
// would walk right into and "succeed" on, rewriting the dependency instead of failing loudly.
const CARGO_WORKSPACE_VERSION = `[package]
name = "app"
version.workspace = true
description = "Camera reports, generated entirely client-side."

[dependencies.serde]
version = "1.0"
features = ["derive"]
`

describe('bumpVersionFiles with [package] version.workspace = true', () => {
  test('throws instead of rewriting the dependency version', () => {
    expect(() =>
      bumpVersionFiles({ pkg: PKG, cargo: CARGO_WORKSPACE_VERSION, conf: CONF }, '0.2.0'),
    ).toThrow()
  })
})
