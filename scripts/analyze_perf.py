import csv
import re
import statistics
import sys
from pathlib import Path


def parse_kv(line: str) -> dict[str, str]:
    out = {}
    for m in re.finditer(r'([a-zA-Z0-9_]+)=("[^"]*"|\S+)', line):
        val = m.group(2).strip('"')
        out[m.group(1)] = val
    return out


def to_float(s: str, default: float = 0.0) -> float:
    try:
        return float(re.sub(r"[^0-9.\-]", "", s))
    except ValueError:
        return default


def main() -> None:
    if len(sys.argv) < 2:
        print("Usage: analyze_perf.py <session.log> [hw.csv]")
        sys.exit(1)

    log_path = Path(sys.argv[1])
    hw_path = Path(sys.argv[2]) if len(sys.argv) > 2 else None

    lines = log_path.read_text(encoding="utf-8", errors="replace").splitlines()

    fps_samples = []
    stalls = []
    gpu_activities = []
    gpu_threads = []
    gpu_memories = []
    fence_latencies = []
    mem_reports = []
    waits = []
    errors_warns = []

    for idx, line in enumerate(lines):
        ts_m = re.match(r"^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z)", line)
        ts = ts_m.group(1) if ts_m else f"line:{idx}"
        kv = parse_kv(line)

        if "frame rate" in line and "game_fps" in kv:
            fps_samples.append((ts, to_float(kv["game_fps"]), to_float(kv.get("speed_pct", "0")), to_float(kv.get("host_fps", "0"))))
        elif "draw stalls" in line:
            stalls.append((ts, kv))
        elif "GPU activity period=" in line:
            gpu_activities.append((ts, kv))
        elif '"GPU thread"' in line or line.rstrip().endswith("GPU thread"):
            gpu_threads.append((ts, kv))
        elif '"GPU memory"' in line or line.rstrip().endswith("GPU memory"):
            gpu_memories.append((ts, kv))
        elif "GPU fence latency" in line:
            fence_latencies.append((ts, kv))
        elif "memory private_mib=" in line:
            mem_reports.append((ts, kv))
        elif " waits:" in line or "period_ms=" in line:
            waits.append((ts, line))
        elif " ERROR " in line or " WARN " in line:
            errors_warns.append((ts, line))

    print("=== PERFORMANCE SUMMARY ===")
    if fps_samples:
        # Ignore initial 0-fps boot ticks before first rendered frame when summarizing active gameplay
        active = [x for x in fps_samples if x[1] > 0.5]
        target = active if active else fps_samples
        g_fps = [x[1] for x in target]
        spd = [x[2] for x in target]
        h_fps = [x[3] for x in target]
        print(f"Active FPS samples ({len(target)} intervals, 5s each):")
        print(f"  Game FPS : min={min(g_fps):.1f}, median={statistics.median(g_fps):.1f}, avg={statistics.mean(g_fps):.1f}, max={max(g_fps):.1f}")
        print(f"  Speed %  : min={min(spd):.0f}%, median={statistics.median(spd):.0f}%, avg={statistics.mean(spd):.1f}%, max={max(spd):.0f}%")
        print(f"  Host FPS : min={min(h_fps):.1f}, median={statistics.median(h_fps):.1f}, avg={statistics.mean(h_fps):.1f}, max={max(h_fps):.1f}")
    else:
        print("No FPS samples recorded yet.")

    if stalls:
        print(f"\n=== LAG / STUTTER WINDOWS ({len(stalls)} seconds flagged) ===")
        total_skipped_sh = sum(int(to_float(s[1].get("skipped_shader", "0"))) for s in stalls)
        total_skipped_pl = sum(int(to_float(s[1].get("skipped_pipeline", "0"))) for s in stalls)
        total_wait_sh = sum(int(to_float(s[1].get("waited_shader", "0"))) for s in stalls)
        total_wait_pl = sum(int(to_float(s[1].get("waited_pipeline", "0"))) for s in stalls)
        worst_ms = max(to_float(s[1].get("worst_frame_ms", "0")) for s in stalls)
        print(f"  Worst single frame gap : {worst_ms:.1f} ms")
        print(f"  Shader stalls total    : skipped={total_skipped_sh}, waited={total_wait_sh}")
        print(f"  Pipeline stalls total  : skipped={total_skipped_pl}, waited={total_wait_pl}")
        slowest = sorted(stalls, key=lambda x: to_float(x[1].get("worst_frame_ms", "0")), reverse=True)[:10]
        print("  Top 10 worst lag seconds:")
        for ts, kv in slowest:
            print(
                f"    [{ts}] worst={to_float(kv.get('worst_frame_ms','0')):.1f}ms | "
                f"presents={kv.get('presents','?')} | "
                f"shader(skip={kv.get('skipped_shader','0')},wait={kv.get('waited_shader','0')},ms={to_float(kv.get('shader_ms','0')):.1f},q={kv.get('translating','0')}) | "
                f"pipe(skip={kv.get('skipped_pipeline','0')},wait={kv.get('waited_pipeline','0')},ms={to_float(kv.get('pipeline_ms','0')):.1f},q={kv.get('building','0')})"
            )

    if gpu_activities:
        print(f"\n=== GPU PIPELINE BOTTLENECK ({len(gpu_activities)} periods) ===")
        for ts, kv in gpu_activities:
            print(
                f"  [{ts}] draws={kv.get('draws','0')} dispatches={kv.get('dispatches','0')} presents={kv.get('presents','0')} | "
                f"longest_call={kv.get('longest_call','?')} | front_idle={kv.get('front_idle','?')} "
                f"front_wait_back={kv.get('front_waiting_for_back','?')} | "
                f"back_idle={kv.get('back_idle','?')} back_wait_host={kv.get('back_waiting_for_host_gpu','?')} | "
                f"traps={kv.get('trap_flushes','0')}"
            )

    if gpu_memories:
        print(f"\n=== VRAM & TEXTURE/BUFFER CACHE PRESSURE ===")
        max_vram = max((to_float(kv.get("vram_used_mib", "0")) for _, kv in gpu_threads), default=0.0)
        budget = max((to_float(kv.get("vram_budget_mib", "0")) for _, kv in gpu_threads), default=0.0)
        max_img = max((to_float(kv.get("cached_image_mib", "0")) for _, kv in gpu_memories), default=0.0)
        max_buf = max((to_float(kv.get("cached_buffer_mib", "0")) for _, kv in gpu_memories), default=0.0)
        tot_ev_img = sum(int(to_float(kv.get("evicted_images", "0"))) for _, kv in gpu_memories)
        tot_cev_img = sum(int(to_float(kv.get("create_evicted_images", "0"))) for _, kv in gpu_memories)
        tot_ev_buf = sum(int(to_float(kv.get("evicted_buffers", "0"))) for _, kv in gpu_memories)
        tot_flush = sum(int(to_float(kv.get("flushed_images", "0"))) for _, kv in gpu_memories)
        tot_sync = sum(int(to_float(kv.get("texture_gpu_syncs", "0"))) for _, kv in gpu_threads)
        tot_dl = sum(int(to_float(kv.get("texture_downloads", "0"))) for _, kv in gpu_threads)
        tot_ul = sum(int(to_float(kv.get("texture_uploads", "0"))) for _, kv in gpu_threads)
        print(f"  Peak Vulkan VRAM used  : {max_vram:.0f} MiB / budget {budget:.0f} MiB")
        print(f"  Peak Cached Textures   : {max_img:.0f} MiB | Peak Cached Buffers: {max_buf:.0f} MiB")
        print(f"  Texture activity       : uploads={tot_ul}, downloads={tot_dl}, gpu_syncs={tot_sync}")
        print(f"  Evictions (thrashing)  : images={tot_ev_img} (create_evicted={tot_cev_img}), flushed={tot_flush}, buffers={tot_ev_buf}")

    if fence_latencies:
        print(f"\n=== GPU FENCE LATENCY ===")
        avg_total = statistics.mean(to_float(kv.get("total_ms", "0")) for _, kv in fence_latencies)
        max_fence = max(to_float(kv.get("longest_ms", "0")) for _, kv in fence_latencies)
        print(f"  Avg fence total_ms={avg_total:.2f} ms | Longest fence={max_fence:.2f} ms")

    if hw_path and hw_path.exists():
        rows = list(csv.DictReader(hw_path.open(encoding="utf-8")))
        if rows:
            max_priv = max(to_float(r["proc_private_mb"]) for r in rows)
            max_ws = max(to_float(r["proc_workingset_mb"]) for r in rows)
            min_free = min(to_float(r["os_free_ram_mb"]) for r in rows)
            max_ded = max(to_float(r["gpu_dedicated_vram_mb"]) for r in rows)
            max_shr = max(to_float(r["gpu_shared_vram_mb"]) for r in rows)
            print("\n=== HOST RAM & GPU MEMORY (Windows Counters) ===")
            print(f"  Lichii Private RAM peak : {max_priv:.1f} MB | Working Set peak: {max_ws:.1f} MB")
            print(f"  OS Free RAM minimum     : {min_free:.1f} MB  (low free RAM < 700 MB triggers Windows paging stutter!)")
            print(f"  GPU Dedicated VRAM peak : {max_ded:.1f} MB  | Shared VRAM peak: {max_shr:.1f} MB (shared > 250 MB = PCIe spill!)")

    if waits:
        print(f"\n=== TOP THREAD WAIT BOTTLENECKS ({len(waits)} samples) ===")
        for ts, line in waits[-10:]:
            print(f"  [{ts}] {line.strip()}")

    if errors_warns:
        print(f"\n=== WARNINGS & ERRORS ({len(errors_warns)}) ===")
        seen = {}
        for _, line in errors_warns:
            msg = re.sub(r"^\S+\s+", "", line)
            seen[msg] = seen.get(msg, 0) + 1
        for msg, count in sorted(seen.items(), key=lambda kv: -kv[1])[:15]:
            print(f"  [{count}x] {msg}")


if __name__ == "__main__":
    main()
