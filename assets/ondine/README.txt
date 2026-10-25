ondine 0.0.3 (https://github.com/wevm/ondine, MIT)

Unmodified copies from the npm package (dist/internal/all.generated.wasm and
the LICENSE), plus an esbuild bundle of dist/Render.js and dist/internal/Wasm.js.

- all.generated.wasm: every diagram type compiled to WebAssembly.
- ondine.js: `export { create } from './Render.js'` bundled with
  `esbuild --bundle --format=iife --global-name=Ondine --minify`.
  The global `Ondine.create({ modules: { all: base64 } })` renders Mermaid
  source to SVG.
- LICENSE: upstream MIT license.

The renderer is embedded in the HTML only when the document has a ```mermaid
block, so pages without diagrams stay small and stay JavaScript-free. Update by
copying new files from the ondine package and rebuilding `ondine.js` with the
command above.
