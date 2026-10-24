// The app's engraved Moon (crates/luna-app/src/moon.rs), fragment by
// fragment in GLSL. Same constants, same geometry, same colour arithmetic.
// One pass per pixel: `prepare` + `render` from the Rust side folded together,
// since the GPU does the whole image at once.
//
// Output is premultiplied: the disc is opaque; the halo, limb and dial are
// light added to whatever lies behind the canvas, with alpha 0.

export const MAP_W = 1024;
export const MAP_H = 512;

export const vertex = /* glsl */ `
void main() {
  gl_Position = vec4(position.xy, 0.0, 1.0);
}
`;

export const fragment = /* glsl */ `
out vec4 fragColor;

uniform vec2 uRes;        // canvas size in pixels (square)
uniform float uSs;        // samples per screen pixel along each axis
uniform float uLit;       // lit fraction, 0 new .. 1 full
uniform float uSpin;      // rotation about the axis, radians
uniform float uProgress;  // share of the dial lit, clockwise from the top
uniform sampler2D uAlbedo;

const float PI = 3.14159265358979;
const float TAU = 6.28318530717959;
const float MAP_W = ${MAP_W}.0;
const float MAP_H = ${MAP_H}.0;

const float RADIUS = 0.36;
const float RING = 1.17;
const float TICKS = 72.0;
const float LINE_PERIOD = 5.2;
const float CROSS_STRETCH = 1.15;

const vec3 BG = vec3(12.0, 15.0, 20.0) / 255.0;
const vec3 MOONLIGHT = vec3(236.0, 239.0, 244.0) / 255.0;
const vec3 LUNA_BLUE = vec3(121.0, 202.0, 234.0) / 255.0;
const vec3 DIM_BLUE = vec3(42.0, 74.0, 92.0) / 255.0;

// moon.rs hash01: the same 32-bit arithmetic, so the stipple is the app's.
float hash01(int ix, int iy) {
  uint h = uint(ix) * 374761393u + uint(iy) * 668265263u;
  h = (h ^ (h >> 13u)) * 1274126177u;
  return float(h & 0xFFFFu) / 65535.0;
}

// An anti-aliased line of \`width\` (share of the period) at \`dist\`.
float line(float dist, float perPx, float width) {
  float edge = clamp((width * 0.5 - dist) * perPx + 0.5, 0.0, 1.0);
  return edge * clamp(width * perPx, 0.0, 1.0);
}

float distanceToLine(float p) {
  return abs(p - floor(p + 0.5));
}

// Bilinear albedo at a map position, wrapping in longitude.
float albedoAt(float u, float v) {
  float x0f = floor(u);
  float y0f = clamp(floor(v), 0.0, MAP_H - 2.0);
  float fx = u - x0f;
  float fy = clamp(v - y0f, 0.0, 1.0);
  // Texel centres: the texture repeats in S, clamps in T.
  vec2 t00 = vec2((x0f + 0.5) / MAP_W, (y0f + 0.5) / MAP_H);
  vec2 t10 = t00 + vec2(1.0 / MAP_W, 0.0);
  vec2 t01 = t00 + vec2(0.0, 1.0 / MAP_H);
  vec2 t11 = t00 + vec2(1.0 / MAP_W, 1.0 / MAP_H);
  float a = mix(texture2D(uAlbedo, t00).r, texture2D(uAlbedo, t10).r, fx);
  float b = mix(texture2D(uAlbedo, t01).r, texture2D(uAlbedo, t11).r, fx);
  return mix(a, b, fy);
}

// Earthshine stipple: one dot per map cell, fixed to the ground.
float dot_(float u, float v, float east, float north, float cell, float albedo) {
  float cu = u / cell;
  float cv = v / cell;
  int wrap = int(max(floor(MAP_W / cell + 0.5), 1.0));
  int ix = int(floor(cu));
  ix = ((ix % wrap) + wrap) % wrap;
  int iy = int(floor(cv));
  if (hash01(ix + 5, iy + 3) >= 0.015 + 0.06 * albedo) return 0.0;
  float jx = 0.2 + 0.6 * hash01(ix, iy);
  float jy = 0.2 + 0.6 * hash01(iy + 91, ix + 17);
  float dx = (cu - floor(cu) - jx) * cell * east;
  float dy = (cv - floor(cv) - jy) * cell * north;
  return clamp(1.2 * uSs - sqrt(dx * dx + dy * dy), 0.0, 1.0);
}

void main() {
  float n = uRes.x;
  float r = n * RADIUS;
  float c = n * 0.5;
  float ss = uSs;
  float period = LINE_PERIOD * ss;
  float limbWidth = 0.7 * ss;
  float texel = r * TAU / MAP_W;
  float cell = 4.0 * ss / texel;

  vec2 p = gl_FragCoord.xy;
  float vx = (p.x - c) / r;
  float vy = (p.y - c) / r;
  float rr = sqrt(vx * vx + vy * vy);
  float limb = clamp(1.0 - abs(rr - 1.0) * r / limbWidth, 0.0, 1.0);

  float lit = clamp(uLit, 0.0, 1.0);
  float phi = acos(clamp(1.0 - 2.0 * lit, -1.0, 1.0));
  vec3 sun = vec3(sin(phi), 0.0, -cos(phi));
  float shift = uSpin / TAU * MAP_W;

  vec3 s = BG;

  if (rr < 1.0) {
    float vz = sqrt(max(1.0 - rr * rr, 0.0));
    // The Moon leans a little; lines follow its latitude.
    float sl = sin(radians(-16.0));
    float cl = cos(radians(-16.0));
    float sx = cl * vx - sl * vy;
    float sy = sl * vx + cl * vy;
    float lon = atan(sx, vz);
    float lat = asin(clamp(sy, -1.0, 1.0));
    float u = ((lon / TAU) + 0.5) * MAP_W - 0.5;
    float v = (0.5 - lat / PI) * MAP_H - 0.5;
    float mainPos = (sy * r + vz * 0.10 * r) / period;
    float sa = sin(radians(58.0));
    float ca = cos(radians(58.0));
    float crossAxis = ca * sx + sa * sy;
    float crossPos = (crossAxis * r + vz * 0.08 * r) / (period * CROSS_STRETCH);
    float dMain = distanceToLine(mainPos);
    float dCross = distanceToLine(crossPos);
    float east = texel * cos(lat);

    float albedo = albedoAt(u + shift, v);
    float mu0 = vx * sun.x + vy * sun.y + vz * sun.z;
    // Lommel-Seeliger, the Moon's own reflectance: flat-bright when full.
    float reflect = mu0 > 0.0 ? 2.0 * mu0 / (mu0 + vz + 1e-6) : 0.0;
    float light = clamp(albedo * reflect * 0.62, 0.0, 1.0);
    float mainLine = line(dMain, period, pow(light, 0.85) * 0.78);
    float shade = clamp((0.09 - light) / 0.09, 0.0, 1.0) * (mu0 > 0.0 ? 1.0 : 0.0);
    float cross = line(dCross, period * CROSS_STRETCH, shade * 0.3)
      * clamp(light * 10.0, 0.0, 1.0) * 0.8;
    float stipple = mu0 <= 0.0 ? dot_(u + shift, v, east, texel, cell, albedo) : 0.0;

    s += (MOONLIGHT - BG) * clamp(mainLine + cross, 0.0, 1.0);
    s += (LUNA_BLUE - BG) * (stipple * 0.35);
    s += (DIM_BLUE - BG) * (limb * 0.8);
    fragColor = vec4(clamp(s, 0.0, 1.0), 1.0);
    return;
  }

  if (rr < RING + 0.08 || limb > 0.0) {
    vec2 dir = vec2(vx, vy) / rr;
    float turn = fract(atan(vx, vy) / TAU);
    float tickGap = abs(turn * TICKS - floor(turn * TICKS + 0.5)) * (TAU * RING * r / TICKS);
    bool inBand = rr > RING - 0.035 && rr < RING + 0.035;
    float tick = inBand ? clamp(ss - tickGap, 0.0, 1.0) : 0.0;
    float hair = clamp(1.0 - abs(rr - RING) * r / (0.6 * ss), 0.0, 1.0);
    float halo = exp(-(rr - 1.0) * 9.0);
    float toward = (dir.x * sun.x + dir.y * sun.y) * 0.5 + 0.5;
    float litSide = pow(clamp(toward, 0.0, 1.0), 2.0);
    bool done = turn <= clamp(uProgress, 0.0, 1.0);
    vec3 glow = (MOONLIGHT + LUNA_BLUE) * 0.5;

    s += (DIM_BLUE - BG) * (limb * 0.8);
    s += (glow - BG) * (halo * litSide * sqrt(lit) * 0.10);
    float unlitTick = done ? 0.0 : tick * 0.55;
    s += (DIM_BLUE - BG) * (hair * 0.45 + unlitTick);
    if (done) s += (LUNA_BLUE - BG) * tick;
    // Only the light added over the background, premultiplied, alpha 0.
    fragColor = vec4(max(clamp(s, 0.0, 1.0) - BG, 0.0), 0.0);
    return;
  }

  fragColor = vec4(0.0);
}
`;
