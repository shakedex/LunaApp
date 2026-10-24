// The live Moon: a square WebGL canvas running the engraved shader.
import {
  ClampToEdgeWrapping,
  GLSL3,
  LinearFilter,
  Mesh,
  NearestFilter,
  NoColorSpace,
  OrthographicCamera,
  PlaneGeometry,
  RepeatWrapping,
  Scene,
  ShaderMaterial,
  TextureLoader,
  Vector2,
  WebGLRenderer,
  type Texture,
} from "three";
import { fragment, vertex } from "./engrave";

export interface MoonState {
  /** Lit fraction of the disc: 0 new, 1 full. */
  lit: number;
  /** Rotation about the Moon's axis, in radians. */
  spin: number;
  /** Share of the orbit dial lit, 0..=1. */
  progress: number;
}

export interface Moon {
  /** Draws one frame. */
  render(state: MoonState): void;
  /** Re-reads the canvas's CSS size. */
  resize(): void;
  dispose(): void;
}

/** The biggest square the Moon is drawn at, in device pixels. */
const MAX_PIXELS = 1800;

/**
 * Sets the shader up on `canvas`, or returns null where WebGL is missing.
 * The canvas is drawn at twice the device's pixels on ordinary screens, so
 * the lines come out antialiased the way the app supersamples them.
 */
export async function createMoon(canvas: HTMLCanvasElement, albedoUrl: string): Promise<Moon | null> {
  let renderer: WebGLRenderer;
  try {
    renderer = new WebGLRenderer({
      canvas,
      alpha: true,
      antialias: false,
      premultipliedAlpha: true,
      powerPreference: "low-power",
    });
  } catch (e) {
    console.warn("Luna: no WebGL, the Moon stays a picture", e);
    return null;
  }
  renderer.setClearColor(0x000000, 0);

  let texture: Texture;
  try {
    texture = await new TextureLoader().loadAsync(albedoUrl);
  } catch (e) {
    console.warn("Luna: the Moon's albedo did not load", e);
    renderer.dispose();
    return null;
  }
  texture.wrapS = RepeatWrapping;
  texture.wrapT = ClampToEdgeWrapping;
  texture.magFilter = NearestFilter;
  texture.minFilter = LinearFilter;
  texture.generateMipmaps = false;
  texture.colorSpace = NoColorSpace;
  texture.flipY = false;
  texture.needsUpdate = true;

  const material = new ShaderMaterial({
    // GLSL ES 3.00: the stipple hash needs unsigned integer arithmetic.
    glslVersion: GLSL3,
    vertexShader: vertex,
    fragmentShader: fragment,
    uniforms: {
      uRes: { value: new Vector2(1, 1) },
      uSs: { value: 1 },
      uLit: { value: 1 },
      uSpin: { value: 0 },
      uProgress: { value: 1 },
      uAlbedo: { value: texture },
    },
    transparent: true,
    depthTest: false,
    depthWrite: false,
  });
  // The material writes premultiplied colour itself; the browser composites.
  material.blending = 0; // NoBlending: write the fragment as is
  const scene = new Scene();
  scene.add(new Mesh(new PlaneGeometry(2, 2), material));
  const camera = new OrthographicCamera(-1, 1, 1, -1, 0, 1);

  let pixels = 0;
  const resize = () => {
    const css = canvas.clientWidth || canvas.width;
    const dpr = window.devicePixelRatio || 1;
    // Two samples per device pixel on 1x screens, one on dense ones.
    const ss = dpr >= 1.5 ? 1 : 2;
    const size = Math.min(MAX_PIXELS, Math.round(css * dpr * ss));
    if (size === pixels) return;
    pixels = size;
    renderer.setPixelRatio(1);
    renderer.setSize(size, size, false);
    material.uniforms.uRes.value.set(size, size);
    material.uniforms.uSs.value = ss;
  };
  resize();

  return {
    render(state) {
      material.uniforms.uLit.value = state.lit;
      material.uniforms.uSpin.value = state.spin;
      material.uniforms.uProgress.value = state.progress;
      renderer.render(scene, camera);
    },
    resize,
    dispose() {
      material.dispose();
      texture.dispose();
      renderer.dispose();
    },
  };
}
