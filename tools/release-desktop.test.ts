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
