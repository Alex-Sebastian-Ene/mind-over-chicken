// Times the learned relief pass compiled to wasm (V8 = Chrome's engine), single thread.
//
//   cargo build --release --target wasm32-unknown-unknown -p moc-diffusion --features wasm-bench
//   node crates/moc-diffusion/tools/bench_wasm.mjs <path/to/moc_diffusion.wasm> [steps] [c0 c1 c2]
import { readFileSync } from "node:fs";
import { performance } from "node:perf_hooks";

const [wasmPath, stepsArg, ...widthArgs] = process.argv.slice(2);
const steps = Number(stepsArg ?? 6);
const [c0, c1, c2] = widthArgs.length === 3 ? widthArgs.map(Number) : [24, 32, 48];
const { instance } = await WebAssembly.instantiate(readFileSync(wasmPath), {});
const { mocd_bench_setup, mocd_bench_run } = instance.exports;

const params = mocd_bench_setup(c0, c1, c2, 2);
console.log(`wasm  widths [${c0}, ${c1}, ${c2}]  params ${params}  steps ${steps}`);
for (const [name, w, h, cell, reps] of [
  ["home world", 320, 180, 2, 3],
  ["galaxy surface", 640, 360, 2, 2],
  ["civ planet", 2560, 1280, 4, 1],
]) {
  let best = Infinity;
  for (let r = 0; r < reps; r++) {
    const t = performance.now();
    mocd_bench_run(w, h, cell, steps, 1 + r);
    best = Math.min(best, performance.now() - t);
  }
  console.log(`${name.padEnd(15)} ${w}x${h}  ${best.toFixed(0)} ms`);
}
