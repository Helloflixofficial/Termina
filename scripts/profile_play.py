import ctypes
import ctypes.wintypes as wt
import datetime
import os
import re
import subprocess
import sys
import threading
import time
from pathlib import Path

DEFAULT_GAME = (
    r"C:\Users\zero\Downloads\Mails\The Legend of Zelda Ocarina of Time Switch 2 NUP Base Game"
    r"\The Legend of Zelda Ocarina of Time [0400486024DEC000][v131072].nup"
)

LIVE_PATTERN = re.compile(
    r"frame rate|draw stalls|GPU activity|GPU thread|GPU memory|GPU fence latency|period_ms=|memory private_mib|ERROR|WARN"
)


class MEMORYSTATUSEX(ctypes.Structure):
    _fields_ = [
        ("dwLength", wt.DWORD),
        ("dwMemoryLoad", wt.DWORD),
        ("ullTotalPhys", ctypes.c_uint64),
        ("ullAvailPhys", ctypes.c_uint64),
        ("ullTotalPageFile", ctypes.c_uint64),
        ("ullAvailPageFile", ctypes.c_uint64),
        ("ullTotalVirtual", ctypes.c_uint64),
        ("ullAvailVirtual", ctypes.c_uint64),
        ("sullAvailExtendedVirtual", ctypes.c_uint64),
    ]


class PROCESS_MEMORY_COUNTERS_EX(ctypes.Structure):
    _fields_ = [
        ("cb", wt.DWORD),
        ("PageFaultCount", wt.DWORD),
        ("PeakWorkingSetSize", ctypes.c_size_t),
        ("WorkingSetSize", ctypes.c_size_t),
        ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
        ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
        ("PagefileUsage", ctypes.c_size_t),
        ("PeakPagefileUsage", ctypes.c_size_t),
        ("PrivateUsage", ctypes.c_size_t),
    ]


def get_os_ram_mb() -> tuple[float, float]:
    st = MEMORYSTATUSEX()
    st.dwLength = ctypes.sizeof(st)
    if ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(st)):
        return st.ullTotalPhys / (1024 * 1024), st.ullAvailPhys / (1024 * 1024)
    return 0.0, 0.0


def get_proc_mem_mb(pid: int) -> tuple[float, float]:
    PROCESS_QUERY_INFORMATION = 0x0400
    PROCESS_VM_READ = 0x0010
    handle = ctypes.windll.kernel32.OpenProcess(
        PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, False, pid
    )
    if not handle:
        return 0.0, 0.0
    try:
        pmc = PROCESS_MEMORY_COUNTERS_EX()
        pmc.cb = ctypes.sizeof(pmc)
        if ctypes.windll.psapi.GetProcessMemoryInfo(
            handle, ctypes.byref(pmc), pmc.cb
        ):
            return pmc.PrivateUsage / (1024 * 1024), pmc.WorkingSetSize / (1024 * 1024)
    finally:
        ctypes.windll.kernel32.CloseHandle(handle)
    return 0.0, 0.0


def hw_monitor(pid: int, hw_csv: Path, stop_event: threading.Event, start_t: float) -> None:
    tp = subprocess.Popen(
        [
            "typeperf",
            r"\GPU Adapter Memory(*)\Dedicated Usage",
            r"\GPU Adapter Memory(*)\Shared Usage",
            "-si",
            "1",
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        bufsize=1,
    )
    ded_indices: list[int] = []
    shr_indices: list[int] = []

    with hw_csv.open("w", encoding="utf-8", newline="") as f:
        f.write(
            "timestamp_iso,elapsed_s,proc_private_mb,proc_workingset_mb,os_free_ram_mb,gpu_dedicated_vram_mb,gpu_shared_vram_mb,cpu_time_s\n"
        )
        f.flush()
        try:
            assert tp.stdout is not None
            for raw in tp.stdout:
                if stop_event.is_set():
                    break
                line = raw.strip()
                if not line.startswith('"'):
                    continue
                cols = [c.strip('"') for c in line.split(",")]
                if not ded_indices and len(cols) > 1 and "PDH-CSV" in cols[0]:
                    for i, name in enumerate(cols):
                        low = name.lower()
                        if "dedicated usage" in low:
                            ded_indices.append(i)
                        elif "shared usage" in low:
                            shr_indices.append(i)
                    continue
                if len(cols) < 2:
                    continue

                priv_mb, ws_mb = get_proc_mem_mb(pid)
                _, free_mb = get_os_ram_mb()

                # Pick highest dedicated usage adapter (discrete GPU: RTX 3050)
                best_ded = 0.0
                best_shr = 0.0
                for idx_pos, di in enumerate(ded_indices):
                    if di < len(cols):
                        try:
                            val = float(cols[di])
                        except ValueError:
                            val = 0.0
                        if val >= best_ded:
                            best_ded = val
                            if idx_pos < len(shr_indices) and shr_indices[idx_pos] < len(cols):
                                try:
                                    best_shr = float(cols[shr_indices[idx_pos]])
                                except ValueError:
                                    best_shr = 0.0

                elapsed = time.monotonic() - start_t
                now_iso = datetime.datetime.now().astimezone().isoformat()
                f.write(
                    f"{now_iso},{elapsed:.1f},{priv_mb:.1f},{ws_mb:.1f},{free_mb:.1f},{best_ded / (1024*1024):.1f},{best_shr / (1024*1024):.1f},0\n"
                )
                f.flush()
        finally:
            if tp.poll() is None:
                tp.terminate()
                try:
                    tp.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    tp.kill()


def main() -> None:
    repo_root = Path(__file__).resolve().parent.parent
    os.chdir(repo_root)

    game_path = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_GAME
    exe = repo_root / "target" / "release" / "lichii.exe"
    if not exe.exists():
        print(f"[ERROR] {exe} not found. Run: cargo build --release -p lichii")
        sys.exit(1)

    # Guard against running two instances simultaneously on an 8 GB RAM system
    while True:
        existing = subprocess.run(
            ["powershell", "-NoProfile", "-Command", "(Get-Process lichii -ErrorAction SilentlyContinue).Id"],
            capture_output=True,
            text=True,
        ).stdout.strip()
        if not existing:
            break
        print(f"[WAIT] Another lichii.exe (PID {existing.replace(chr(10), ',')}) is already running.")
        print("       Close that game window first so this profiler can record a clean single-instance run...")
        time.sleep(2)

    log_dir = repo_root / "perf_logs"
    log_dir.mkdir(exist_ok=True)
    ts = datetime.datetime.now().strftime("%Y%m%d_%H%M%S")
    session_log = log_dir / f"session_{ts}.log"
    hw_csv = log_dir / f"hw_{ts}.csv"
    report_txt = log_dir / f"report_{ts}.txt"

    total_ram_mb, free_ram_mb = get_os_ram_mb()
    gpu_info = subprocess.run(
        [
            "powershell",
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_VideoController | ForEach-Object { \"$($_.Name) ($([math]::Round($_.AdapterRAM/1MB)) MB VRAM)\" }",
        ],
        capture_output=True,
        text=True,
    ).stdout.strip().replace("\n", " | ")

    header = [
        "=== HARDWARE BASELINE ===",
        f"Timestamp         : {datetime.datetime.now().astimezone().isoformat()}",
        f"Usable System RAM : {total_ram_mb:.0f} MB (Free at launch: {free_ram_mb:.0f} MB)",
        f"GPUs              : {gpu_info}",
        f"Executable        : {exe}",
        f"Game              : {game_path}",
        "RUST_LOG          : info,waits=debug,lichii_gpu::activity=debug",
        "=========================",
    ]

    print("=" * 68)
    print(" Termina Gameplay Profiler & Bottleneck Recorder")
    print("=" * 68)
    print(f" Session Log : {session_log}")
    print(f" Hardware CSV: {hw_csv}")
    print(f" Report File : {report_txt}")
    for line in header:
        print(line)
    print("\n[LIVE] Starting emulator... Play from one area to another, then close the game window.")
    print("[LIVE] Showing FPS, stalls, VRAM/RAM, GPU activity & thread waits below:\n")

    env = os.environ.copy()
    env["RUST_LOG"] = "info,waits=debug,lichii_gpu::activity=debug"

    args = [str(exe)]
    if game_path and Path(game_path).exists():
        args.append(game_path)

    start_t = time.monotonic()
    proc = subprocess.Popen(
        args,
        cwd=repo_root,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        encoding="utf-8",
        errors="replace",
        bufsize=1,
    )

    stop_event = threading.Event()
    mon = threading.Thread(target=hw_monitor, args=(proc.pid, hw_csv, stop_event, start_t), daemon=True)
    mon.start()

    with session_log.open("w", encoding="utf-8") as log_f:
        for line in header:
            log_f.write(line + "\n")
        log_f.flush()
        assert proc.stdout is not None
        for line in proc.stdout:
            log_f.write(line)
            log_f.flush()
            if LIVE_PATTERN.search(line):
                sys.stdout.write(line)
                sys.stdout.flush()

    rc = proc.wait()
    stop_event.set()
    mon.join(timeout=3)

    print("\n" + "=" * 68)
    print(f" Game exited (code {rc}). Analyzing recorded session...")
    print("=" * 68)

    analyzer = Path(__file__).resolve().parent / "analyze_perf.py"
    rep = subprocess.run(
        [sys.executable, str(analyzer), str(session_log), str(hw_csv)],
        capture_output=True,
        text=True,
    )
    print(rep.stdout)
    if rep.stderr:
        print(rep.stderr)
    report_txt.write_text(rep.stdout, encoding="utf-8")
    print(f"Saved full report to: {report_txt}")


if __name__ == "__main__":
    main()
