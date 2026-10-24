import {
  CustomBlending,
  LinearFilter,
  Mesh,
  OneFactor,
  OneMinusSrcAlphaFactor,
  OrthographicCamera,
  PlaneGeometry,
  RepeatWrapping,
  Scene,
  ShaderMaterial,
  TextureLoader,
  Vector3,
  WebGLRenderer,
} from "three";
import { fragment, vertex } from "./engrave.glsl";
import { phaseToSunDirection } from "./phase";

export interface Moon {
  /** Lit share of the disc, 0 (new) to 1 (full). */
  setPhase(lit: number): void;
  /** Share of the orbit dial lit, 0 to 1. */
  setProgress(p: number): void;
  /** The Moon's square in viewport CSS pixels: top-left corner and side. */
  setRect(x: number, y: number, size: number): void;
  dispose(): void;
}

/** Lines in the app sit 5.2 px apart on a 300 px Moon; a larger Moon gets slightly wider spacing. */
const periodFor = (size: number) => 5.2 * Math.max(1, size / 300) ** 0.55;
const tickFor = (size: number) => Math.max(1, size / 300) * 1.1;

/**
 * Luna's Moon on `canvas` (fixed, full viewport). Resolves to null without WebGL.
 * `onLost` runs if the GPU context goes away, so the page can show the still frame again.
 */
export async function createMoon(canvas: HTMLCanvasElement, onLost: () => void): Promise<Moon | null> {
  let renderer: WebGLRenderer;
  try {
    renderer = new WebGLRenderer({ canvas, alpha: true, premultipliedAlpha: true, antialias: false });
  } catch {
    return null;
  }
  const albedo = await new TextureLoader().loadAsync("/img/moon-albedo.webp");
  albedo.wrapS = RepeatWrapping;
  albedo.minFilter = LinearFilter;
  albedo.generateMipmaps = false;
  // Rows run north to south, as the app reads them.
  albedo.flipY = false;

  const uniforms = {
    uAlbedo: { value: albedo },
    uSize: { value: 1 },
    uSun: { value: new Vector3(0, 0, 1) },
    uLit: { value: 1 },
    uProgress: { value: 1 },
    uPeriod: { value: 5.2 },
    uTick: { value: 1 },
    uSpin: { value: 0 },
  };
  const material = new ShaderMaterial({
    uniforms,
    vertexShader: vertex,
    fragmentShader: fragment,
    transparent: true,
    depthTest: false,
    blending: CustomBlending,
    blendSrc: OneFactor,
    blendDst: OneMinusSrcAlphaFactor,
    premultipliedAlpha: true,
  });
  const mesh = new Mesh(new PlaneGeometry(1, 1), material);
  const scene = new Scene();
  scene.add(mesh);
  const camera = new OrthographicCamera(0, 1, 0, -1, -1, 1);

  let rect = { x: 0, y: 0, size: 0 };
  let queued = false;
  const draw = () => {
    queued = false;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const w = window.innerWidth;
    const h = window.innerHeight;
    renderer.setPixelRatio(dpr);
    renderer.setSize(w, h, false);
    camera.right = w;
    camera.bottom = -h;
    camera.updateProjectionMatrix();
    mesh.scale.set(rect.size, rect.size, 1);
    mesh.position.set(rect.x + rect.size / 2, -(rect.y + rect.size / 2), 0);
    uniforms.uSize.value = rect.size * dpr;
    uniforms.uPeriod.value = periodFor(rect.size) * dpr;
    uniforms.uTick.value = tickFor(rect.size) * dpr;
    renderer.render(scene, camera);
  };
  const request = () => {
    if (!queued) {
      queued = true;
      requestAnimationFrame(draw);
    }
  };

  canvas.addEventListener("webglcontextlost", (e) => {
    e.preventDefault();
    onLost();
  });
  window.addEventListener("resize", request);

  return {
    setPhase(lit) {
      const [x, y, z] = phaseToSunDirection(lit);
      uniforms.uSun.value.set(x, y, z);
      uniforms.uLit.value = Math.min(1, Math.max(0, lit));
      request();
    },
    setProgress(p) {
      uniforms.uProgress.value = Math.min(1, Math.max(0, p));
      request();
    },
    setRect(x, y, size) {
      rect = { x, y, size };
      request();
    },
    dispose() {
      window.removeEventListener("resize", request);
      material.dispose();
      albedo.dispose();
      renderer.dispose();
    },
  };
}
