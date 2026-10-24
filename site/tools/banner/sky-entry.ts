// The site's star chart behind the banner, drawn twice over: at the site's
// strength it all but disappears at thumbnail size.
import { runSky } from "../../src/scripts/sky";

document.querySelectorAll<HTMLCanvasElement>("canvas.sky").forEach((c) => runSky(c));
