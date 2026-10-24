// Luna's engraved Moon as a fragment shader: a per-pixel port of
// crates/luna-app/src/moon.rs (MoonRenderer::render and prepare). Keep the two
// in step: same albedo, lighting, line model, stipple and orbit dial.

export const vertex = /* glsl */ `
varying vec2 vUv;
void main() {
  vUv = uv;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

export const fragment = /* glsl */ `
precision highp float;
varying vec2 vUv;

uniform sampler2D uAlbedo;
uniform float uSize;      // the Moon's square, device pixels
uniform vec3 uSun;        // view-space sun direction
uniform float uLit;       // lit share of the disc, 0..1
uniform float uProgress;  // share of the dial lit, 0..1
uniform float uPeriod;    // device pixels between engraved lines
uniform float uTick;      // tick width, device pixels
uniform float uSpin;      // rotation about the Moon's axis, radians

const float PI = 3.14159265;
const float TAU = 6.28318531;
const float RADIUS = 0.36;   // the Moon's radius as a share of the square
const float RING = 1.17;     // the dial's radius, in Moon radii
const float TICKS = 72.0;
const float CROSS_STRETCH = 1.15;
const vec3 BG = vec3(12.0, 15.0, 20.0) / 255.0;
const vec3 MOONLIGHT = vec3(236.0, 239.0, 244.0) / 255.0;
const vec3 LUNA_BLUE = vec3(121.0, 202.0, 234.0) / 255.0;
const vec3 DIM_BLUE = vec3(42.0, 74.0, 92.0) / 255.0;

float distToLine(float p) { return abs(p - floor(p + 0.5)); }

float lineAt(float dist, float perPx, float width) {
  float edge = clamp((width * 0.5 - dist) * perPx + 0.5, 0.0, 1.0);
  return edge * clamp(width * perPx, 0.0, 1.0);
}

float hash01(vec2 c) {
  return fract(sin(dot(c, vec2(127.1, 311.7))) * 43758.5453);
}

void main() {
  float r = uSize * RADIUS;
  vec2 p = (vUv - 0.5) * uSize;           // pixels from the centre, y up
  vec2 v2 = p / r;
  float rr = length(v2);
  float limb = clamp(1.0 - abs(rr - 1.0) * r / 0.9, 0.0, 1.0);

  // Around the disc: limb, halo and the orbit dial, all added light.
  vec3 around = vec3(0.0);
  if (rr > 0.97) {
    vec2 dir = v2 / max(rr, 1e-4);
    float turn = fract(atan(v2.x, v2.y) / TAU);
    float k = turn * TICKS;
    float gap = abs(k - floor(k + 0.5)) * (TAU * RING * r / TICKS);
    float inBand = step(abs(rr - RING), 0.035);
    float tick = inBand * clamp(uTick - gap, 0.0, 1.0);
    float hair = clamp(1.0 - abs(rr - RING) * r / 0.6, 0.0, 1.0);
    float halo = exp(-(rr - 1.0) * 9.0) * step(1.0, rr);
    float toward = clamp(dot(dir, uSun.xy) * 0.5 + 0.5, 0.0, 1.0);
    float done = step(turn, uProgress) * step(0.0001, uProgress);
    vec3 glow = (MOONLIGHT + LUNA_BLUE) * 0.5;
    around += (DIM_BLUE - BG) * limb * 0.8;
    around += (glow - BG) * halo * toward * toward * sqrt(uLit) * 0.10;
    around += (DIM_BLUE - BG) * (hair * 0.45 + (1.0 - done) * tick * 0.55);
    around += (LUNA_BLUE - BG) * done * tick;
  }

  // The disc: latitude lines that thicken with the light, cross-hatching in
  // the shade by the terminator, earthshine stipple on the night side.
  float cover = clamp((1.0 - rr) * r + 0.5, 0.0, 1.0);
  vec3 disc = BG;
  if (cover > 0.0) {
    float rc = min(rr, 0.9999);
    vec2 vd = v2 * (rc / max(rr, 1e-4));
    float vz = sqrt(max(1.0 - rc * rc, 0.0));
    const float SL = -0.27563736;  // sin(-16 deg)
    const float CL = 0.96126170;
    const float SA = 0.84804810;   // sin(58 deg)
    const float CA = 0.52991926;
    float sx = CL * vd.x - SL * vd.y;
    float sy = SL * vd.x + CL * vd.y;
    float lon = atan(sx, vz);
    float lat = asin(clamp(sy, -1.0, 1.0));
    vec2 uv = vec2(lon / TAU + 0.5 + uSpin / TAU, 0.5 - lat / PI);
    float albedo = texture2D(uAlbedo, uv).r;

    vec3 n = vec3(vd, vz);
    float mu0 = dot(n, uSun);
    float reflectance = mu0 > 0.0 ? 2.0 * mu0 / (mu0 + vz + 1e-6) : 0.0;
    float light = clamp(albedo * reflectance * 0.62, 0.0, 1.0);

    float mainP = (sy * r + vz * 0.10 * r) / uPeriod;
    float crossP = ((CA * sx + SA * sy) * r + vz * 0.08 * r) / (uPeriod * CROSS_STRETCH);
    float mainL = lineAt(distToLine(mainP), uPeriod, pow(light, 0.85) * 0.78);
    float shade = clamp((0.09 - light) / 0.09, 0.0, 1.0) * step(0.0, mu0);
    float crossL = lineAt(distToLine(crossP), uPeriod * CROSS_STRETCH, shade * 0.3)
      * clamp(light * 10.0, 0.0, 1.0) * 0.8;

    float stipple = 0.0;
    if (mu0 <= 0.0) {
      float texel = r * TAU / 1024.0;          // pixels per map texel
      float cell = 4.0 / texel;                // a 4-pixel cell, in texels
      vec2 t = uv * vec2(1024.0, 512.0) / cell;
      vec2 c = floor(t);
      if (hash01(c + vec2(5.0, 3.0)) < 0.015 + 0.06 * albedo) {
        vec2 j = 0.2 + 0.6 * vec2(hash01(c), hash01(c.yx + vec2(91.0, 17.0)));
        vec2 d = (t - c - j) * cell * vec2(texel * cos(lat), texel);
        stipple = clamp(1.2 - length(d), 0.0, 1.0);
      }
    }

    disc += (MOONLIGHT - BG) * clamp(mainL + crossL, 0.0, 1.0);
    disc += (LUNA_BLUE - BG) * stipple * 0.35;
    disc += (DIM_BLUE - BG) * limb * 0.8;
  }

  // Premultiplied: the disc is opaque, everything around it is added light.
  gl_FragColor = vec4(disc * cover + around * (1.0 - cover), cover);
}
`;
